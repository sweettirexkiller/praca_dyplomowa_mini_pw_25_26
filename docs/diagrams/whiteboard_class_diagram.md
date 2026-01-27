classDiagram
    class AppView {
        +WhiteboardState whiteboard
        +DocBackend backend
    }

    class WhiteboardState {
        +ColorImage image
        +Option~TextureHandle~ texture
        +Color32 stroke_color
        +f32 stroke_width
        +Vec~Point~ current_stroke
        +Tool tool
        +Option~ColorImage~ background
    }

    class Tool {
        <<enumeration>>
        Pen
        Eraser
    }

    class Point {
        +i32 x
        +i32 y
    }
    
    class ColorImage {
        +[usize; 2] size
        +Vec~Color32~ pixels
    }

    AppView *-- WhiteboardState : contains
    WhiteboardState *-- Tool : has selected
    WhiteboardState *-- Point : composes current_stroke
    WhiteboardState *-- ColorImage : backing buffer
