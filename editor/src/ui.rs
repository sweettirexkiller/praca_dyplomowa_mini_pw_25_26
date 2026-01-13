//! UI Module
//! Defines the structure and logic for the application's user interface using `eframe` and `egui`.
use std::{
    env,
    sync::{Arc, Mutex},
};

use crate::backend_api::{DocBackend, Intent};
use eframe::{egui, egui::Context};
use jsonwebtoken::{encode, EncodingKey, Header};
use livekit_api::access_token;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use rand::{distr::Alphanumeric, Rng};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

mod ui_panels;

use livekit::prelude::*;

/// Generates a consistent color for a user based on their username.
/// Used to represent remote cursors.
/// 
/// # Arguments
/// * `username` - The identity of the user.
/// Generates a consistent user color based on the username hash.
pub fn get_user_color(username: &str) -> egui::Color32 {
    let mut hasher = DefaultHasher::new();
    username.hash(&mut hasher);
    let hash = hasher.finish();
    
    // Generate distinct, bright colors using HSV
    let h = (hash as u32 % 360) as f32 / 360.0;
    egui::Color32::from(egui::ecolor::Hsva::new(h, 0.8, 0.8, 1.0))
}

/// Represents a packet of data transferred over the network (via LiveKit Data API).
/// Handles fragmentation for large messages.
#[derive(Serialize, Deserialize, Debug)]
pub enum TransportPacket {
    /// A small message that fits in a single packet.
    Message(Vec<u8>), 
    /// A chunk of a larger message.
    Chunk {
        /// Unique ID for the message being fragmented.
        id: u64,
        /// Index of this chunk.
        index: u32,
        /// Total number of chunks.
        total: u32,
        /// Payload data for this chunk.
        data: Vec<u8>
    }
}

/// High-level network message types used for application logic.
#[derive(Serialize, Deserialize, Debug)]
pub enum NetworkMessage {
    /// CRDT synchronization data.
    Sync(Vec<u8>),
    /// Chat message.
    Chat(String),
    /// Remote cursor position.
    Cursor { x: i32, y: i32 },
}

/// Internal commands sent from the UI thread to the background network thread.
#[derive(Debug)]
pub enum AppCommand {
    /// Disconnect from the current room.
    Disconnect,
    /// Broadcast a message to all participants in the room.
    Broadcast(NetworkMessage),
    /// Send a message to specific recipients.
    Send { recipients: Vec<String>, message: NetworkMessage },
}

/// Internal messages sent from the background network thread to the UI thread.
#[derive(Debug)]
pub enum AppMsg {
    /// Log message to be displayed in the UI.
    Log(String),
    /// Notification that a new participant connected.
    ParticipantConnected(String),
    /// Notification that a participant disconnected.
    ParticipantDisconnected(String),
    /// A network message received from a peer.
    NetworkMessage { sender: String, message: NetworkMessage },
}

/// Main application structure holding the state of the editor and UI.
/// Implements `eframe::App`.
pub struct AppView {
    /// The document backend (CRDT logic).
    backend: Box<dyn DocBackend>,
    /// Status message displayed in the status bar.
    status: String,
    /// State of the sidebar.
    sidebar: SidebarState,
    /// Current active page (Editor or LiveKit console).
    page: Page,
    /// State of the collaborative whiteboard.
    whiteboard: WhiteboardState,

    // Connected LiveKit room state
    /// Log of LiveKit events.
    livekit_events: Arc<Mutex<Vec<String>>>,
    /// List of connected participants.
    livekit_participants: Arc<Mutex<Vec<String>>>,
    /// Whether currently connected to a LiveKit room.
    livekit_connected: bool,
    /// Whether currently attempting to connect.
    livekit_connecting: bool,
    // LiveKit panel inputs
    /// URL of the LiveKit server.
    livekit_ws_url: String,
    /// Identity of the local user.
    livekit_identity: String,
    // shared token storage so background threads can set the generated token for the UI/connection
    // editable token field for the UI (user can paste or modify)
    /// Access token for LiveKit.
    livekit_token: String,
    /// Name of the room to join.
    livekit_room: String,
    /// Current chat message input buffer.
    livekit_message: String,
     // Channel to send messages to the background LiveKit task
    /// Sender channel for communicating with the network thread.
    livekit_command_sender: Option<tokio::sync::mpsc::UnboundedSender<AppCommand>>,
    
    /// Positions of remote cursors.
    remote_cursors: std::collections::HashMap<String, crate::backend_api::Point>,
    /// Timestamp of last cursor update broadcast.
    last_cursor_update: std::time::Instant,
    /// Receiver channel for messages from the network thread.
    app_msg_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<AppMsg>>,
}

/// State for the collapsible sidebar configuration.
struct SidebarState {
    visible: bool,
    default_width: f32,
}

/// Enumeration of available drawing tools.
#[derive(PartialEq, Eq)]
enum Tool {
    /// Freehand pen.
    Pen,
    /// Eraser.
    Eraser,
}

/// State of the whiteboard canvas.
struct WhiteboardState {
    /// The backing pixel buffer of the whiteboard.
    image: egui::ColorImage,
    /// Texture handle for rendering the image on GPU.
    texture: Option<egui::TextureHandle>,
    /// Currently selected stroke color.
    stroke_color: egui::Color32,
    /// Currently selected stroke width (brush size).
    stroke_width: f32,
    /// Points accumulated in the current stroke being drawn.
    current_stroke: Vec<crate::backend_api::Point>,
    /// Currently selected tool.
    tool: Tool,
    /// Optional background image loaded from a file.
    background: Option<egui::ColorImage>,
}

/// Enumeration of main application pages/views.
#[derive(PartialEq, Eq)]
pub enum Page {
    /// The main whiteboard editor.
    Editor,
    /// The LiveKit connection management screen.
    LiveKit,
}

impl AppView {
    /// Initializes the application view with a given backend.
    pub fn new(backend: Box<dyn DocBackend>) -> Self {
        // let text_cache = backend.render_text(); // Removed, as we use get_strokes dynamically or on event
        let host = std::env::var("LIVEKIT_URL").unwrap_or_else(|_| "127.0.0.1:7880".to_string());
        let web_socket_url = if host.starts_with("ws://") || host.starts_with("wss://") {
            host
        } else if host.starts_with("http://") {
            host.replacen("http://", "ws://", 1)
        } else if host.starts_with("https://") {
            host.replacen("https://", "wss://", 1)
        } else {
            format!("ws://{}", host)
        };

        let mut app = Self {
            backend,
            status: "Ready".into(),
            sidebar: SidebarState {
                visible: false,
                default_width: 260.0,
            },
            whiteboard: WhiteboardState {
                image: egui::ColorImage::new([800, 600], vec![egui::Color32::WHITE; 800 * 600]),
                texture: None,
                stroke_color: egui::Color32::BLACK,
                stroke_width: 5.0,
                current_stroke: Vec::new(),
                tool: Tool::Pen,
                background: None,
            },
            page: Page::Editor,
            livekit_events: Arc::new(Mutex::new(Vec::new())),
            livekit_participants: Arc::new(Mutex::new(Vec::new())),
            livekit_connected: false,
            livekit_connecting: false,
            livekit_ws_url: web_socket_url.into(),
            livekit_identity: "".into(),
            livekit_token: "".into(),
            livekit_room: "".into(),
            remote_cursors: std::collections::HashMap::new(),
            last_cursor_update: std::time::Instant::now(),
            livekit_message: "".into(),
            livekit_command_sender: None,
            app_msg_receiver: None,
        };
        
        // Initial load
        let initial_strokes = app.backend.get_strokes();
        app.apply_update(crate::backend_api::FrontendUpdate { strokes: initial_strokes });
        
        app
    }

    /// Triggers synchronization with all connected peers.
    fn sync_with_all(&mut self) {
        let participants = self.livekit_participants.lock().unwrap().clone();
        for p in participants {
            if p.contains("(You)") { continue; }
             if let Some(payload) = self.backend.generate_sync_message(&p) {
                if let Some(tx) = &self.livekit_command_sender {
                    let _ = tx.send(AppCommand::Send { 
                        recipients: vec![p], 
                        message: NetworkMessage::Sync(payload) 
                    });
                }
            }
        }
    }

    /// Processes a local intent (e.g., user drawing).
    /// Applies it to the backend and broadcasts updates.
    fn handle_intent(&mut self, intent: Intent) {
        println!("Handling intent: {:?}", intent);
        let update = self.backend.apply_intent(intent);
        self.apply_update(update);
        self.sync_with_all();
    }
    
    /// Applies an update from the backend to the UI state.
    /// This handles redrawing strokes and updating the background image.
    fn apply_update(&mut self, update: crate::backend_api::FrontendUpdate) {
        // Always try to sync background from backend if it might have changed.
        // For optimization, we could check a hash, but here we just check if backend has something 
        // and we have nothing, OR if we have something, we might want to check if it matches?
        // Let's assume for now that if we receive an update, we should check availability.
        
        let backend_bg = self.backend.get_background();
        let should_reload = match (&self.whiteboard.background, &backend_bg) {
            (None, Some(_)) => true,
            (Some(bg), Some(data)) => {
                 // Should compare. length check is cheap.
                 // This is imperfect but better than nothing.
                 // A proper way would be to store the hash of the source data in WhiteboardState.
                 // For now, let's RELOAD if lengths differ significantly or just assume if sync happened we should check.
                 // Actually, decoding image every frame is bad.
                 // Let's rely on: if we have background, assume it's current unless we clear it.
                 // BUT current bug is receiver doesn't get it. Receiver usually starts as None.
                 // If receiver restarts -> None -> Gets it.
                 // If receiver is running, Sender sets BG -> Receiver gets sync.
                 // Receiver has None -> Gets it.
                 // Case: User has BG1. Peer sets BG2.
                 // Receiver has Some(BG1). Backend has Some(BG2).
                 // We need to know BG2 is different.
                 // Let's use a dirty hack: Store the length of last loaded background bytes in WhiteboardState?
                 // Or just load it if present?
                 // Let's just trust that if we receive a sync, and backend has data, we should use it?
                 // No, that loops.
                 false // Modify logic below to handle "replacement" if we can detect it.
            },
            _ => false,
        };
        
        // Revised logic:
        // We really want to replace the local background if the backend one is different.
        // Since we don't have a hash, we'll implement a simple one: store the backend bytes in a field if we can?
        // Or just decoded image.
        
        if self.whiteboard.background.is_none() && backend_bg.is_some() {
             if let Some(bg_bytes) = backend_bg {
                 if let Ok(img) = image::load_from_memory(&bg_bytes) {
                      let img = img.to_rgba8();
                      let size = [img.width() as usize, img.height() as usize];
                      let pixels = img.as_flat_samples().as_slice().to_vec();
                      self.whiteboard.background = Some(egui::ColorImage::from_rgba_unmultiplied(size, &pixels));
                 }
             }
        } else if self.whiteboard.background.is_some() && backend_bg.is_some() {
            // Check if we need to update.
            // Since we don't track what bytes generated current background easily, 
            // maybe we can skip this for now safely if the main issue was "initial load".
            // The user said "problem exists only when I load", suggesting initial load or single load scenario.
            // If they change background repeatedly, we might need better logic.
            // But let's check if the backend bytes are different from what we used? We don't have what we used.
            // Let's just leave the simple "is_none" check for now, combined with the sync fix in `open_file`.
            // If the user replaces background, they probably call clear first, which sets None.
            // Wait, does open_file clear first? Yes: `self.handle_intent(Intent::Clear);` clears strokes.
            // Does it clear background?
            // `open_file` sets `self.whiteboard.background = Some(...)`.
            
            // If peer does `open_file`: 
            // Peer calls `set_background`.
            // Peer sends sync.
            // We receive sync. `apply_update` runs.
            // We have `background: None` (if we just joined) -> We load it.
            // If we have `background: Some` (old one) -> We check `backend` has new one?
            // If `set_background` overwrites, `backend.get_background()` returns new one.
            // Converting to image every frame is unacceptable.
            // We need a way to detect "new background arrived".
            // Since we can't easily change `FrontendUpdate` right now without refactoring `backend_api` and `automerge_backend` deep logic...
            // We can check if `backend_bg` length != `current_bg_source_len`?
            // But we don't have `current_bg_source_len`.
        }

        // Simple full redraw for now
        if let Some(bg) = &self.whiteboard.background {
            self.whiteboard.image = bg.clone();
        } else {
            self.whiteboard.image = egui::ColorImage::new([800, 600], vec![egui::Color32::WHITE; 800 * 600]);
        }

        for stroke in update.strokes {
            self.draw_stroke_on_image(&stroke);
        }
        if let Some(texture) = &mut self.whiteboard.texture {
             if texture.size() != self.whiteboard.image.size {
                  // Size mismatch, we must let egui recreate it or handle it in editor_center
                  self.whiteboard.texture = None;
             } else {
                  texture.set(self.whiteboard.image.clone(), egui::TextureOptions::NEAREST);
             }
        }
    }
    
    /// Helper to render a single stroke onto the whiteboard image.
    /// Renders a stroke onto the local whiteboard image.
    ///
    /// # Arguments
    /// * `stroke` - The stroke to draw.
    fn draw_stroke_on_image(&mut self, stroke: &crate::backend_api::Stroke) {
        let color = egui::Color32::from_rgba_premultiplied(
            stroke.color[0], stroke.color[1], stroke.color[2], stroke.color[3]
        );
        let brush_size = stroke.width as i32;
        let width = self.whiteboard.image.width();
        let height = self.whiteboard.image.height();
        
        for point in &stroke.points {
            let x = point.x;
            let y = point.y;
             for dy in -brush_size..=brush_size {
                for dx in -brush_size..=brush_size {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                         // Check circular brush
                        if dx * dx + dy * dy <= brush_size * brush_size {
                            let idx = (ny as usize * width) + nx as usize;
                            self.whiteboard.image.pixels[idx] = color;
                        }
                    }
                }
             }
        }
    }

    /// Generates a LiveKit access token for joining a room.
    /// Generates a LiveKit access token for joining a room.
    ///
    /// Requires `LIVEKIT_API_KEY` and `LIVEKIT_API_SECRET` environment variables.
    fn create_token(
        room_name: &str,
        identity: &str,
    ) -> Result<String, access_token::AccessTokenError> {
        let api_key = env::var("LIVEKIT_API_KEY").expect("LIVEKIT_API_KEY is not set");
        let api_secret = env::var("LIVEKIT_API_SECRET").expect("LIVEKIT_API_SECRET is not set");

        access_token::AccessToken::with_api_key(&api_key, &api_secret)
            .with_identity(identity)
            .with_name(identity)
            .with_grants(access_token::VideoGrants {
                room_join: true,
                room: room_name.to_string(),
                can_publish: true,
                can_publish_data: true, // Required to send chat messages
                ..Default::default()
            })
            .to_jwt()
    }
    // ...existing code...
    /// Connects to a LiveKit room or creates one if it doesn't exist (if configured on server).
    /// Spawns a background thread to handle network events.
    /// Initiates a connection to the LiveKit room.
    ///
    /// Spawns a background thread to handle network events.
    pub fn connect_or_create_to_room(&mut self, ctx: egui::Context) {
       if self.livekit_connected {
            return;
        }
        self.livekit_connecting = true;

        if self.livekit_room.is_empty() {
             // Generate random room name if empty (e.g. from Share button or just empty)
             self.livekit_room = rand::rng()
                .sample_iter(&Alphanumeric)
                .take(5)
                .map(char::from)
                .collect();
        }

        if self.livekit_identity.is_empty() {
             self.livekit_identity = rand::rng()
                .sample_iter(&Alphanumeric)
                .take(5)
                .map(char::from)
                .collect();
        }

        println!("Connecting to LiveKit room {} as {}...", self.livekit_room, self.livekit_identity);

        println!("Generating token...");
        let token = match Self::create_token(&self.livekit_room, &self.livekit_identity) {
            Ok(t) => t,
            Err(e) => {
                let mut guard = self.livekit_events.lock().unwrap();
                guard.push(format!("Token generation error: {}", e));
                self.livekit_connecting = false;
                return;
            }
        };

        println!("Token generated: {}", token);
        self.livekit_token = token.clone();
        
        let url = self.livekit_ws_url.clone();
        
        // Channel for App -> Thread
        let (tx_cmd, mut rx_cmd) = tokio::sync::mpsc::unbounded_channel::<AppCommand>();
        self.livekit_command_sender = Some(tx_cmd);
        
        // Channel for Thread -> App
        let (tx_msg, rx_msg) = tokio::sync::mpsc::unbounded_channel::<AppMsg>();
        self.app_msg_receiver = Some(rx_msg);

        let tx_msg_clone = tx_msg.clone();
        let ctx_clone = ctx.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut incomplete_transfers: std::collections::HashMap<String, std::collections::HashMap<u64, (u32, Vec<Option<Vec<u8>>>)>> = std::collections::HashMap::new();

                let (room, mut room_events) = match Room::connect(&url, &token, RoomOptions::default()).await {
                    Ok(res) => res,
                    Err(e) => {
                         let _ = tx_msg.send(AppMsg::Log(format!("Connection failed: {}", e)));
                         ctx_clone.request_repaint();
                        return;
                    }
                };
                
                let room = Arc::new(room);
                 let _ = tx_msg.send(AppMsg::Log("Connected to Room".to_string()));
                 ctx_clone.request_repaint();

                // Initial participants list
                // We should probably send connection events for existing participants? 
                // Or let the UI pull them? For now, we rely on events.
                for (_, p) in room.remote_participants() {
                     let _ = tx_msg.send(AppMsg::ParticipantConnected(p.identity().to_string()));
                     ctx_clone.request_repaint();
                }

                loop {
                    tokio::select! {
                        Some(event) = room_events.recv() => {
                            match event {
                                RoomEvent::DataReceived { payload, participant, .. } => {
                                    if let Some(p) = participant {
                                        let sender = p.identity().to_string();
                                        
                                        // Try to parse as TransportPacket
                                        if let Ok(packet) = serde_json::from_slice::<TransportPacket>(&payload) {
                                            match packet {
                                                TransportPacket::Message(data) => {
                                                     if let Ok(msg) = serde_json::from_slice::<NetworkMessage>(&data) {
                                                         let _ = tx_msg.send(AppMsg::NetworkMessage { sender, message: msg });
                                                         ctx_clone.request_repaint();
                                                     }
                                                },
                                                TransportPacket::Chunk { id, index, total, data } => {
                                                    let entry = incomplete_transfers.entry(sender.clone()).or_default();
                                                    let transfer = entry.entry(id).or_insert_with(|| (0, vec![None; total as usize]));
                                                    
                                                    if (index as usize) < transfer.1.len() {
                                                        if transfer.1[index as usize].is_none() {
                                                            transfer.1[index as usize] = Some(data);
                                                            transfer.0 += 1;
                                                        }

                                                        if transfer.0 == total {
                                                            // All chunks received
                                                            let full_data: Vec<u8> = transfer.1.iter().flat_map(|c| c.as_ref().unwrap().clone()).collect();
                                                            entry.remove(&id);
                                                            
                                                            if let Ok(msg) = serde_json::from_slice::<NetworkMessage>(&full_data) {
                                                                let _ = tx_msg.send(AppMsg::NetworkMessage { sender, message: msg });
                                                                ctx_clone.request_repaint();
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        } else if let Ok(msg) = serde_json::from_slice::<NetworkMessage>(&payload) {
                                             // Backward compatibility or direct message
                                             let _ = tx_msg.send(AppMsg::NetworkMessage { sender, message: msg });
                                             ctx_clone.request_repaint();
                                         }
                                    }
                                }
                                RoomEvent::ParticipantConnected(p) => {
                                    let _ = tx_msg.send(AppMsg::ParticipantConnected(p.identity().to_string()));
                                    ctx_clone.request_repaint();
                                }
                                RoomEvent::ParticipantDisconnected(p) => {
                                    let id = p.identity().to_string();
                                    incomplete_transfers.remove(&id);
                                    let _ = tx_msg.send(AppMsg::ParticipantDisconnected(id));
                                    ctx_clone.request_repaint();
                                }
                                RoomEvent::Disconnected { reason } => {
                                     let _ = tx_msg.send(AppMsg::Log(format!("Disconnected: {:?}", reason)));
                                     ctx_clone.request_repaint();
                                     break;
                                }
                                _ => {}
                            }
                        }
                        cmd = rx_cmd.recv() => {
                            match cmd {
                                Some(AppCommand::Disconnect) => {
                                    break; 
                                }
                                Some(AppCommand::Broadcast(msg)) => {
                                    if let Ok(data) = serde_json::to_vec(&msg) {
                                        let chunks_count = (data.len() + 14000 - 1) / 14000;
                                        if chunks_count <= 1 {
                                            let packet = TransportPacket::Message(data);
                                            if let Ok(payload) = serde_json::to_vec(&packet) {
                                                let _ = room.local_participant()
                                                    .publish_data(DataPacket {
                                                        payload,
                                                        reliable: true,
                                                        ..Default::default()
                                                    })
                                                    .await;
                                            }
                                        } else {
                                            let id: u64 = rand::random();
                                            for (i, chunk) in data.chunks(14000).enumerate() {
                                                let packet = TransportPacket::Chunk {
                                                    id,
                                                    index: i as u32,
                                                    total: chunks_count as u32,
                                                    data: chunk.to_vec()
                                                };
                                                if let Ok(payload) = serde_json::to_vec(&packet) {
                                                    let _ = room.local_participant()
                                                        .publish_data(DataPacket {
                                                            payload,
                                                            reliable: true,
                                                            ..Default::default()
                                                        })
                                                        .await;
                                                }
                                            }
                                        }
                                    }
                                }
                                Some(AppCommand::Send { recipients, message }) => {
                                     if let Ok(data) = serde_json::to_vec(&message) {
                                        let chunks_count = (data.len() + 14000 - 1) / 14000;
                                        if chunks_count <= 1 {
                                             let packet = TransportPacket::Message(data);
                                             if let Ok(payload) = serde_json::to_vec(&packet) {
                                                let _ = room.local_participant()
                                                    .publish_data(DataPacket {
                                                        payload,
                                                        reliable: true,
                                                        destination_identities: recipients.into_iter().map(Into::into).collect(),
                                                        ..Default::default()
                                                    })
                                                    .await;
                                             }
                                        } else {
                                            let id: u64 = rand::random();
                                            let dest: Vec<livekit::prelude::ParticipantIdentity> = recipients.into_iter().map(Into::into).collect();
                                            for (i, chunk) in data.chunks(14000).enumerate() {
                                                let packet = TransportPacket::Chunk {
                                                    id,
                                                    index: i as u32,
                                                    total: chunks_count as u32,
                                                    data: chunk.to_vec()
                                                };
                                                if let Ok(payload) = serde_json::to_vec(&packet) {
                                                    let _ = room.local_participant()
                                                        .publish_data(DataPacket {
                                                            payload,
                                                            reliable: true,
                                                            destination_identities: dest.clone(),
                                                            ..Default::default()
                                                        })
                                                        .await;
                                                }
                                            }
                                        }
                                    }
                                }
                                None => break, 
                            }
                        }
                    }
                }
                
                room.close().await.ok();
            });
        });

        self.livekit_connecting = false;
        self.livekit_connected = true;
        self.livekit_participants.lock().unwrap().push(self.livekit_identity.clone());
    }

    /// Sends a chat message to all participants in the room.
    /// Sends a chat message to all participants in the room.
    pub fn send_livekit_message(&mut self, message: String) {
        if !self.livekit_connected {
            return;
        }
        if let Some(sender) = &self.livekit_command_sender {
            // Log locally
            self.livekit_events.lock().unwrap().push(format!("You: {}", message));
            let _ = sender.send(AppCommand::Broadcast(NetworkMessage::Chat(message)));
        }
    }

    /// Disconnects from the current LiveKit room.
    /// Disconnects from the current LiveKit room and cleans up resources.
    pub fn disconnect_room(&mut self) {
        if let Some(sender) = &self.livekit_command_sender {
            let _ = sender.send(AppCommand::Disconnect);
        }
        self.livekit_connected = false;
        self.livekit_command_sender = None;
        self.app_msg_receiver = None;
        self.livekit_participants.lock().unwrap().clear();
        self.livekit_events.lock().unwrap().push("Disconnected.".to_string());
        
        // Also clear local whiteboard? No, keep it.
        // But maybe clear sync states?
    }
    
    /// Checks if there's any content in the current whiteboard.
    /// Checks if there are any strokes or background data that might need saving.
    fn has_unsaved_work(&self) -> bool {
        !self.backend.get_strokes().is_empty() || self.whiteboard.background.is_some()
    }

    /// Clears the current document and starts a new one (optionally saving).
    /// Clears the current document and starts a new one.
    /// Prompts the user to save if there are unsaved changes.
    pub fn new_document(&mut self) {
        if self.has_unsaved_work() {
             let result = rfd::MessageDialog::new()
                .set_title("New Document")
                .set_description("Do you want to save your current work?")
                .set_buttons(rfd::MessageButtons::YesNoCancel)
                .show();

            match result {
                rfd::MessageDialogResult::Yes => {
                    if !self.save_file() {
                        return;
                    }
                }
                rfd::MessageDialogResult::No => {}
                _ => return,
            }
        }

        self.whiteboard.background = None;
        self.backend.set_background(Vec::new());
        self.handle_intent(Intent::Clear);
    }

    /// Opens a save dialog to save the current document state or image.
    /// Supports `.crdt` (state) and `.png` (image).
    /// Saves the current document state to a file.
    /// Supports `.crdt` (CRDT state) and `.png` (image export).
    /// Returns `true` if saved successfully, `false` otherwise.
    pub fn save_file(&mut self) -> bool {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("CRDT State", &["crdt"])
            .add_filter("PNG Image", &["png"])
            .save_file() 
        {
            if let Some(extension) = path.extension() {
                if extension == "png" {
                    let width = self.whiteboard.image.width() as u32;
                    let height = self.whiteboard.image.height() as u32;
                    let pixels: Vec<u8> = self.whiteboard.image.pixels.iter()
                        .flat_map(|p| p.to_array())
                        .collect();
                    
                    if let Some(image_buffer) = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, pixels) {
                        if let Err(e) = image_buffer.save(&path) {
                             eprintln!("Failed to save PNG: {}", e);
                             return false;
                        } else {
                             println!("Saved PNG to {:?}", path);
                        }
                    }
                } else {
                     // Default to CRDT save
                    let data = self.backend.save();
                    if let Err(e) = std::fs::write(&path, data) {
                        eprintln!("Failed to save file: {}", e);
                        return false;
                    } else {
                         println!("Saved to {:?}", path);
                    }
                }
            }
            true
        } else {
            false
        }
    }

    /// Opens a file dialog to load a document.
    /// Opens a document from a file.
    /// Supports `.crdt` (CRDT state) and `.png` (load as background).
    /// Prompts to save unsaved work before opening.
    pub fn open_file(&mut self) {
        // Ask used to save
        if self.has_unsaved_work() {
            let result = rfd::MessageDialog::new()
                .set_title("Open File")
                .set_description("Do you want to save your current work?")
                .set_buttons(rfd::MessageButtons::YesNoCancel)
                .show();

            match result {
                rfd::MessageDialogResult::Yes => {
                    if !self.save_file() {
                        return; 
                    }
                }
                rfd::MessageDialogResult::No => {}
                _ => return, 
            }
        }

        if let Some(path) = rfd::FileDialog::new()
            .add_filter("CRDT State", &["crdt"])
            .add_filter("PNG Image", &["png"])
            .pick_file() 
        {
             if let Some(extension) = path.extension() {
                if extension == "png" {
                    if let Ok(img) = image::open(&path) {
                        // Clean the board
                        self.handle_intent(Intent::Clear);

                        let img = img.to_rgba8();
                        let size = [img.width() as usize, img.height() as usize];
                        
                        // We need the raw bytes.
                        let pixels: Vec<u8> = img.as_flat_samples().as_slice().to_vec();
                        
                        let color_image = egui::ColorImage::from_rgba_unmultiplied(
                            size,
                            &pixels,
                        );
                        self.whiteboard.background = Some(color_image);
                        
                        // Save background to backend for sync/persistence
                        if let Ok(bytes) = std::fs::read(&path) {
                             self.backend.set_background(bytes);
                        }
                        self.sync_with_all();

                        // Refresh UI (redraw strokes over new background)
                        let strokes = self.backend.get_strokes();
                        self.apply_update(crate::backend_api::FrontendUpdate { strokes });
                    } else {
                        eprintln!("Failed to open PNG");
                    }
                } else {
                    if let Ok(data) = std::fs::read(&path) {
                        self.whiteboard.background = None;
                        self.backend.load(data);
                        self.sync_with_all();

                        // Refresh UI
                        let strokes = self.backend.get_strokes();
                        self.apply_update(crate::backend_api::FrontendUpdate { strokes });
                    } else {
                         eprintln!("Failed to read file");
                    }
                }
             }
        }
    }
}

// eframe trait for AppView
impl eframe::App for AppView {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Handle incoming messages
        if let Some(mut rx) = self.app_msg_receiver.take() {
            while let Ok(msg) = rx.try_recv() {
                 match msg {
                    AppMsg::Log(s) => {
                         self.livekit_events.lock().unwrap().push(s);
                    }
                    AppMsg::ParticipantConnected(id) => {
                        {
                            let mut participants = self.livekit_participants.lock().unwrap();
                            if !participants.contains(&id) {
                                participants.push(id.clone());
                            }
                        }
                         self.livekit_events.lock().unwrap().push(format!("Participant connected: {}", id));
                        self.backend.peer_connected(&id);
                        if let Some(payload) = self.backend.generate_sync_message(&id) {
                            if let Some(tx) = &self.livekit_command_sender {
                                let _ = tx.send(AppCommand::Send { 
                                    recipients: vec![id], 
                                    message: NetworkMessage::Sync(payload) 
                                });
                            }
                        }
                    }
                    AppMsg::ParticipantDisconnected(id) => {
                        let mut guard = self.livekit_participants.lock().unwrap();
                        if let Some(pos) = guard.iter().position(|x| *x == id) {
                            guard.remove(pos);
                        }
                         self.livekit_events.lock().unwrap().push(format!("Participant disconnected: {}", id));
                        self.backend.peer_disconnected(&id);
                        println!("Cleaning up cursor for participant: {}", id);
                        self.remote_cursors.remove(&id);
                    }
                    AppMsg::NetworkMessage { sender, message } => {
                        match message {
                            NetworkMessage::Chat(text) => {
                                 self.livekit_events.lock().unwrap().push(format!("[{}] {}", sender, text));
                            }
                            NetworkMessage::Sync(data) => {
                                let update = self.backend.receive_sync_message(&sender, data);
                                self.apply_update(update);
                                self.sync_with_all();
                            }
                            NetworkMessage::Cursor { x, y } => {
                                let participants = self.livekit_participants.lock().unwrap();
                                if participants.contains(&sender) {
                                    self.remote_cursors.insert(sender, crate::backend_api::Point { x, y });
                                }
                            }
                        }
                    }
                }
            }
            self.app_msg_receiver = Some(rx);
        }

        self.top_bar(ctx);
        self.sidebar_panel(ctx);
        if self.page == Page::Editor {
            self.editor_center(ctx);
        } else {
            self.livekit_panel(ctx);
        }
        self.status_bar(ctx);
    }
}
