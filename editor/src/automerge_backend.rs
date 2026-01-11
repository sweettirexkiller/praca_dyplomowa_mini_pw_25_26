use crate::backend_api::{DocBackend, FrontendUpdate, Intent, Stroke};
use automerge::{AutoCommit, ReadDoc, transaction::Transactable, ObjType, Value, ScalarValue, ROOT};

pub struct AutomergeBackend {
    doc: AutoCommit,
}

impl AutomergeBackend {
    pub fn new() -> Self {
        let mut doc = AutoCommit::new();
        // Ensure "strokes" list exists
        if doc.get(ROOT, "strokes").unwrap().is_none() {
             doc.put_object(ROOT, "strokes", ObjType::List).ok();
        }
        Self { doc }
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
}
