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
}
