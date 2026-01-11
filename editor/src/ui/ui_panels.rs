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
                // self.open_file();
            }
            if i.modifiers.command && i.key_pressed(egui::Key::S) {
                // self.save();
            }
        });
    }

    pub fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("topbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("☰ Menu").clicked() {
                    self.sidebar.visible = !self.sidebar.visible;
                }

                // ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                //     ui.label(format!("Cursor Position: {}", self.editor.cursor));
                // });
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
                if ui.button("+ New").clicked() {
                    self.handle_intent(crate::backend_api::Intent::Clear);
                    self.status = "New whiteboard".into();
                    self.sidebar.docs.push("untitled.png".into());
                    self.sidebar.selected = self.sidebar.docs.len() - 1;
                }

                // new: open LiveKit page
                if ui.button("Open LiveKit").clicked() {
                    self.page = Page::LiveKit;
                }

                for (i, name) in self.sidebar.docs.iter().enumerate() {
                    let selected = self.sidebar.selected == i;
                    if ui.selectable_label(selected, name).clicked() {
                        self.sidebar.selected = i;
                        // Hook up: load different doc later
                    }
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
                        self.connect_or_create_to_room();
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
                        let color = self.whiteboard.stroke_color;

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
                    let stroke = crate::backend_api::Stroke {
                        points: self.whiteboard.current_stroke.clone(),
                        color: self.whiteboard.stroke_color.to_array(),
                        width: self.whiteboard.stroke_width,
                    };
                    self.handle_intent(crate::backend_api::Intent::Draw(stroke));
                    self.whiteboard.current_stroke.clear();
                 }
            }
        });
    }

    pub fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            // ui.horizontal_wrapped(|ui| {
            //     ui.label(&self.status);
            //     ui.separator();
            //     ui.label(format!("Length: {}", self.editor.text.chars().count()));
            //     ui.separator();
            //     ui.label(if self.sidebar.visible {
            //         "Sidebar: visible"
            //     } else {
            //         "Sidebar: hidden"
            //     });
            // });
        });
    }
}
