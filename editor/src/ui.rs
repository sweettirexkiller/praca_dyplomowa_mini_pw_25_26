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

pub fn get_user_color(username: &str) -> egui::Color32 {
    let mut hasher = DefaultHasher::new();
    username.hash(&mut hasher);
    let hash = hasher.finish();
    
    // Generate distinct, bright colors using HSV
    let h = (hash as u32 % 360) as f32 / 360.0;
    egui::Color32::from(egui::ecolor::Hsva::new(h, 0.8, 0.8, 1.0))
}

#[derive(Serialize, Deserialize, Debug)]
pub enum NetworkMessage {
    Sync(Vec<u8>),
    Chat(String),
    Cursor { x: i32, y: i32 },
}

#[derive(Debug)]
pub enum AppCommand {
    Disconnect,
    Broadcast(NetworkMessage),
    Send { recipients: Vec<String>, message: NetworkMessage },
}

#[derive(Debug)]
pub enum AppMsg {
    Log(String),
    ParticipantConnected(String),
    ParticipantDisconnected(String),
    NetworkMessage { sender: String, message: NetworkMessage },
}

pub struct AppView {
    backend: Box<dyn DocBackend>,
    status: String,
    sidebar: SidebarState,
    page: Page,
    whiteboard: WhiteboardState,

    // Connected LiveKit room state
    livekit_events: Arc<Mutex<Vec<String>>>,
    livekit_participants: Arc<Mutex<Vec<String>>>,
    livekit_connected: bool,
    livekit_connecting: bool,
    // LiveKit panel inputs
    livekit_ws_url: String,
    livekit_identity: String,
    // shared token storage so background threads can set the generated token for the UI/connection
    // editable token field for the UI (user can paste or modify)
    livekit_token: String,
    livekit_room: String,
    livekit_message: String,
     // Channel to send messages to the background LiveKit task
    livekit_command_sender: Option<tokio::sync::mpsc::UnboundedSender<AppCommand>>,
    
    remote_cursors: std::collections::HashMap<String, crate::backend_api::Point>,
    last_cursor_update: std::time::Instant,
    app_msg_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<AppMsg>>,
}

struct SidebarState {
    visible: bool,
    default_width: f32,
}

#[derive(PartialEq, Eq)]
enum Tool {
    Pen,
    Eraser,
}

struct WhiteboardState {
    image: egui::ColorImage,
    texture: Option<egui::TextureHandle>,
    stroke_color: egui::Color32,
    stroke_width: f32,
    current_stroke: Vec<crate::backend_api::Point>,
    tool: Tool,
    background: Option<egui::ColorImage>,
}

#[derive(PartialEq, Eq)]
pub enum Page {
    Editor,
    LiveKit,
}

impl AppView {
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

    fn handle_intent(&mut self, intent: Intent) {
        println!("Handling intent: {:?}", intent);
        let update = self.backend.apply_intent(intent);
        self.apply_update(update);
        self.sync_with_all();
    }
    
    fn apply_update(&mut self, update: crate::backend_api::FrontendUpdate) {
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
             texture.set(self.whiteboard.image.clone(), egui::TextureOptions::NEAREST);
        }
    }
    
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
                                    let sender = participant.map(|p| p.identity().to_string()).unwrap_or("Unknown".to_string());
                                     if let Ok(msg) = serde_json::from_slice::<NetworkMessage>(&payload) {
                                         let _ = tx_msg.send(AppMsg::NetworkMessage { sender, message: msg });
                                     } else {
                                        // Try as legacy chat string
                                        let text = String::from_utf8_lossy(&payload).to_string();
                                        let _ = tx_msg.send(AppMsg::NetworkMessage { 
                                            sender: sender.clone(), 
                                            message: NetworkMessage::Chat(text) 
                                        });
                                     }
                                     ctx_clone.request_repaint();
                                }
                                RoomEvent::ParticipantConnected(p) => {
                                    let _ = tx_msg.send(AppMsg::ParticipantConnected(p.identity().to_string()));
                                    ctx_clone.request_repaint();
                                }
                                RoomEvent::ParticipantDisconnected(p) => {
                                    let _ = tx_msg.send(AppMsg::ParticipantDisconnected(p.identity().to_string()));
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
                                    if let Ok(payload) = serde_json::to_vec(&msg) {
                                        let _ = room.local_participant()
                                            .publish_data(DataPacket {
                                                payload,
                                                reliable: true,
                                                ..Default::default()
                                            })
                                            .await;
                                    }
                                }
                                Some(AppCommand::Send { recipients, message }) => {
                                     if let Ok(payload) = serde_json::to_vec(&message) {
                                        let _ = room.local_participant()
                                            .publish_data(DataPacket {
                                                payload,
                                                reliable: true,
                                                destination_identities: recipients.into_iter().map(Into::into).collect(),
                                                ..Default::default()
                                            })
                                            .await;
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
    
    fn has_unsaved_work(&self) -> bool {
        !self.backend.get_strokes().is_empty() || self.whiteboard.background.is_some()
    }

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
        self.handle_intent(Intent::Clear);
    }

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
                                self.remote_cursors.insert(sender, crate::backend_api::Point { x, y });
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
