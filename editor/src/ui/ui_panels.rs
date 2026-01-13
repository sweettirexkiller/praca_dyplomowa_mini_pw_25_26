use super::*;
use eframe::{egui, egui::Context};
use egui::Key;

impl AppView {
    pub fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.modifiers.command && i.key_pressed(egui::Key::Backslash) {
                self.sidebar.visible = !self.sidebar.visible;
            }
            if i.modifiers.command && i.key_pressed(egui::Key::O) {
                self.open_file();
            }
            if i.modifiers.command && i.key_pressed(egui::Key::S) {
                self.save_file();
            }
        });
    }

    pub fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("topbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("☰ Menu").clicked() {
                    self.sidebar.visible = !self.sidebar.visible;
                }

                if ui.button("Save").clicked() {
                    self.save_file();
                }
                
                if ui.button("Open").clicked() {
                    self.open_file();
                }

                ui.separator();
                
                ui.label("Tool:");
                ui.radio_value(&mut self.whiteboard.tool, Tool::Pen, "✏ Pen");
                ui.radio_value(&mut self.whiteboard.tool, Tool::Eraser, "🧹 Eraser");
                
                ui.separator();
                
                if self.whiteboard.tool == Tool::Pen {
                    ui.label("Color:");
                    ui.color_edit_button_srgba(&mut self.whiteboard.stroke_color);
                }
                
                ui.separator();
                
                ui.label("Size:");
                ui.add(egui::Slider::new(&mut self.whiteboard.stroke_width, 1.0..=50.0));
            });
        });
    }

    pub fn sidebar_panel(&mut self, ctx: &egui::Context) {
        if !self.sidebar.visible {
            return;
        }
        egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(self.sidebar.default_width)
            .show(ctx, |ui| {
                if self.livekit_connected {
                    ui.colored_label(egui::Color32::GREEN, format!("Connected: {}", self.livekit_room));
                    if ui.button("Disconnect from Session").clicked() {
                        self.disconnect_room();
                    }
                } else {
                    if ui.button("Share").clicked() {
                        self.livekit_room = "".into(); // Force new name generation
                        self.connect_or_create_to_room(ctx.clone());
                        self.page = Page::LiveKit;
                    }
                    
                    ui.horizontal(|ui| {
                        ui.label("Room:");
                        ui.text_edit_singleline(&mut self.livekit_room);
                    });
                    if !self.livekit_room.is_empty() {
                         if ui.button("Join Session").clicked() {
                             self.connect_or_create_to_room(ctx.clone());
                             self.page = Page::LiveKit;
                        }
                    }
                }
                
                ui.separator();

                // new: open LiveKit page
                if ui.button("Open LiveKit Console").clicked() {
                    self.page = Page::LiveKit;
                }
            });
    }

    pub fn livekit_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    if ui.button("Back to Editor").clicked() {
                        self.page = Page::Editor;
                    }
                    ui.label(if self.livekit_connecting {
                        "Connecting..."
                    } else {
                        "LiveKit"
                    });
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Room:");
                    ui.text_edit_singleline(&mut self.livekit_room);
                });
                ui.horizontal(|ui| {
                    ui.label("Identity:");
                    ui.text_edit_singleline(&mut self.livekit_identity);
                });

                ui.separator();
                // Create room via Admin API (Cloud / Enterprise only)
                if self.livekit_connected {
                    if ui.button("Disconnect").clicked() {
                        self.disconnect_room();
                    }
                } else {
                    if ui.button("Connect").clicked() {
                        self.connect_or_create_to_room(ctx.clone());
                    }
                }

                ui.separator();

                ui.heading("Events:");
                let events = {
                    let guard = self.livekit_events.lock().unwrap();
                    guard.clone()
                };
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for ev in events.iter().rev() {
                        ui.label(ev);
                    }
                });
                // if connected to the room: Area where messages
                // can be typed and sent displays
                ui.separator();

                if self.livekit_connected {
                    ui.heading("Participants:");
                    let participants = {
                        let guard = self.livekit_participants.lock().unwrap();
                        guard.clone()
                    };
                    egui::ScrollArea::vertical()
                        .id_salt("participants_list") // Add unique ID
                        .max_height(100.0)
                        .show(ui, |ui| {
                            for p in participants {
                                ui.label(format!("• {}", p));
                            }
                        });
                    ui.separator();
                }

                // message input + send button (visible when not connecting)
                if self.livekit_connected {
                    ui.horizontal(|ui| {
                        ui.label("Message:");
                        ui.text_edit_singleline(&mut self.livekit_message);
                        if ui.button("Send").clicked() {
                            // locally record the outgoing message
                            {
                                let mut guard = self.livekit_events.lock().unwrap();
                                guard.push(format!("You: {}", self.livekit_message));
                            }
                            self.send_livekit_message(self.livekit_message.clone());
                            self.livekit_message.clear();
                        }
                    });
                } else {
                    ui.label("Connect to a room to send and see participants messages.");
                }
            });
        });
    }
    pub fn editor_center(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // keep shortcuts here so they work even when sidebar hidden
            self.handle_shortcuts(ctx);

            if self.whiteboard.texture.is_none() {
                self.whiteboard.texture = Some(ui.ctx().load_texture(
                    "whiteboard",
                    self.whiteboard.image.clone(),
                    egui::TextureOptions::NEAREST,
                ));
            }

            let texture = self.whiteboard.texture.as_mut().unwrap();

            // Display the image
            // We want to handle clicks on the image.
            // Using a sense of drag triggers response on drag
            let image_response = ui.add(egui::Image::new(&*texture).sense(egui::Sense::drag()));

            // Handle drawing
            if image_response.hovered() || image_response.dragged() {
                 if let Some(pointer_pos) = ui.input(|i| i.pointer.interact_pos()) {
                     let rect = image_response.rect;
                     if rect.contains(pointer_pos) {
                         let rel_pos = pointer_pos - rect.min;
                         let width = self.whiteboard.image.width();
                         let height = self.whiteboard.image.height();
                         let x = ((rel_pos.x / rect.width()) * width as f32) as i32;
                         let y = ((rel_pos.y / rect.height()) * height as f32) as i32;
                         
                         // Broadcast cursor if time passed
                         if self.livekit_connected && self.last_cursor_update.elapsed() > std::time::Duration::from_millis(50) {
                             if let Some(sender) = &self.livekit_command_sender {
                                 let _ = sender.send(AppCommand::Broadcast(NetworkMessage::Cursor { x, y }));
                                 self.last_cursor_update = std::time::Instant::now();
                             }
                         }
                     }
                 }
            }

            if image_response.dragged() || image_response.clicked() {
                if let Some(pointer_pos) = ui.input(|i| i.pointer.interact_pos()) {
                    let rect = image_response.rect;
                    if rect.contains(pointer_pos) {
                        let rel_pos = pointer_pos - rect.min;
                        let width = self.whiteboard.image.width();
                        let height = self.whiteboard.image.height();

                        // Map scaled image coordinates to actual pixel coordinates
                        let x = ((rel_pos.x / rect.width()) * width as f32) as i32;
                        let y = ((rel_pos.y / rect.height()) * height as f32) as i32;
                        
                        // Add point to current stroke
                        self.whiteboard.current_stroke.push(crate::backend_api::Point { x, y });

                        let brush_size = self.whiteboard.stroke_width as i32;
                        let color = if self.whiteboard.tool == Tool::Eraser {
                            egui::Color32::WHITE
                        } else {
                            self.whiteboard.stroke_color
                        };

                        let mut changed = false;
                        for dy in -brush_size..=brush_size {
                            for dx in -brush_size..=brush_size {
                                let nx = x + dx;
                                let ny = y + dy;
                                if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                                    // Check circular brush
                                    if dx * dx + dy * dy <= brush_size * brush_size {
                                        let idx = (ny as usize * width) + nx as usize;
                                        if self.whiteboard.image.pixels[idx] != color {
                                            self.whiteboard.image.pixels[idx] = color;
                                            changed = true;
                                        }
                                    }
                                }
                            }
                        }

                        if changed {
                            // Update texture
                            texture.set(self.whiteboard.image.clone(), egui::TextureOptions::NEAREST);
                        }
                    }
                }
            }
            
            if image_response.drag_stopped() {
                 if !self.whiteboard.current_stroke.is_empty() {
                    let color = if self.whiteboard.tool == Tool::Eraser {
                        egui::Color32::WHITE
                    } else {
                        self.whiteboard.stroke_color
                    };

                    let stroke = crate::backend_api::Stroke {
                        points: self.whiteboard.current_stroke.clone(),
                        color: color.to_array(),
                        width: self.whiteboard.stroke_width,
                    };
                    self.handle_intent(crate::backend_api::Intent::Draw(stroke));
                    self.whiteboard.current_stroke.clear();
                 }
            }

            // Render remote cursors
            let painter = ui.painter();
            let rect = image_response.rect;
            let width = self.whiteboard.image.width() as f32;
            let height = self.whiteboard.image.height() as f32;
            
            for (user, point) in &self.remote_cursors {
                let rx = (point.x as f32 / width) * rect.width();
                let ry = (point.y as f32 / height) * rect.height();
                let pos = rect.min + egui::Vec2::new(rx, ry);
                
                let color = crate::ui::get_user_color(user);
                painter.circle_filled(pos, 5.0, color);
                painter.text(pos + egui::Vec2::new(8.0, 8.0), egui::Align2::LEFT_TOP, user, egui::FontId::proportional(12.0), color);
            }
        });
    }

    pub fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                
                if self.livekit_connected {
                    ui.separator();
                    ui.label("Participants:");
                    
                    let participants = {
                        let guard = self.livekit_participants.lock().unwrap();
                        guard.clone()
                    };

                    for p in participants {
                        let color = crate::ui::get_user_color(&p);
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 4.0, color);
                        ui.label(&p);
                    }
                }
            });
        });
    }
}
