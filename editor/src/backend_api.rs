//! Backend API - boundary between editor and CRDT logic.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub points: Vec<Point>,
    pub color: [u8; 4],
    pub width: f32,
}

/// intencja uzytkownika w edytorze
#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    Draw(Stroke),
    Clear,
}

/// Aktualizacja dla frontendu - zwracana przez backend po zastosowaniu intencji
#[derive(Debug, Clone, PartialEq)]
pub struct FrontendUpdate {
    pub strokes: Vec<Stroke>,
}

// pusta aktualizacja
impl FrontendUpdate {
    pub fn empty() -> Self {
        Self {
            strokes: Vec::new(),
        }
    }
}

///backend sluzy do zarzadzania dokumentem i synchronizacji
///
/// Trait for document backend - to jest cos w stylu interfejsu, ktory musi byc zaimplementowany
///  przez kazdy backend (narzucone jest ze to moze byc zaimplementowane tylko dla
///  struktur ktore sa Send)
pub trait DocBackend: Send {
    // ta metoda dostaje "intencje" z edytora i zwraca aktualizacje dla edytora
    fn apply_intent(&mut self, intent: Intent) -> FrontendUpdate;
    fn get_strokes(&self) -> Vec<Stroke>;
    
    // Sync methods
    fn peer_connected(&mut self, peer_id: &str);
    fn peer_disconnected(&mut self, peer_id: &str);
    fn receive_sync_message(&mut self, peer_id: &str, message: Vec<u8>) -> FrontendUpdate;
    fn generate_sync_message(&mut self, peer_id: &str) -> Option<Vec<u8>>;

    // Persistence
    fn save(&mut self) -> Vec<u8>;
    fn load(&mut self, data: Vec<u8>);

    // Background
    fn set_background(&mut self, data: Vec<u8>);
    fn get_background(&self) -> Option<Vec<u8>>;
}

pub struct SimpleBackend;

impl SimpleBackend {
    pub fn new() -> Self {
        SimpleBackend
    }
}

impl DocBackend for SimpleBackend {
    fn apply_intent(&mut self, _intent: Intent) -> FrontendUpdate {
        FrontendUpdate::empty()
    }

    fn get_strokes(&self) -> Vec<Stroke> {
        Vec::new()
    }

    fn peer_connected(&mut self, _peer_id: &str) {}
    fn peer_disconnected(&mut self, _peer_id: &str) {}
    fn receive_sync_message(&mut self, _peer_id: &str, _message: Vec<u8>) -> FrontendUpdate {
        FrontendUpdate::empty()
    }
    fn generate_sync_message(&mut self, _peer_id: &str) -> Option<Vec<u8>> {
         None
    }

    fn save(&mut self) -> Vec<u8> {
        Vec::new()
    }

    fn load(&mut self, _data: Vec<u8>) {}

    fn set_background(&mut self, _data: Vec<u8>) {}
    fn get_background(&self) -> Option<Vec<u8>> { None }
}


pub struct MockBackend {
    strokes: Vec<Stroke>,
}

// implementacja traitu Default dla MockBackend, ktory zmusza do implementacji metody default
impl Default for MockBackend {
    fn default() -> Self {
        Self {
            strokes: Vec::new(),
        }
    }
}

// implementujaemy trait DocBackend dla MockBackend
// backend bedzie musial byc w przyszlosci podmieniony
// zmuszamy do implementacji apply_intent i render_text
impl DocBackend for MockBackend {
    // kiedy dostaniemy intencje, to zaktualizujemy tekst i zwrocimy aktualizacje
    fn apply_intent(&mut self, intent: Intent) -> FrontendUpdate {

        match intent {
            Intent::Draw(stroke) => self.strokes.push(stroke),
            Intent::Clear => self.strokes.clear(),
        }
        FrontendUpdate {
            strokes: self.strokes.clone(),
        }
    }

    fn get_strokes(&self) -> Vec<Stroke> {
        self.strokes.clone()
    }

    fn peer_connected(&mut self, _peer_id: &str) {}
    fn peer_disconnected(&mut self, _peer_id: &str) {}
    fn receive_sync_message(&mut self, _peer_id: &str, _message: Vec<u8>) -> FrontendUpdate {
         FrontendUpdate {
            strokes: self.strokes.clone(),
        }
    }
    fn generate_sync_message(&mut self, _peer_id: &str) -> Option<Vec<u8>> {
        None
    }

    fn save(&mut self) -> Vec<u8> {
        Vec::new()
    }

    fn load(&mut self, _data: Vec<u8>) {}

    fn set_background(&mut self, _data: Vec<u8>) {}
    fn get_background(&self) -> Option<Vec<u8>> { None }
}
