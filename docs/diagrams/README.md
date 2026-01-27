# Application Diagrams

This directory contains UML diagrams documenting the architecture and flow of the collaborative editor.

## Structures
- [UI Class Diagram](ui_class_diagram.md) - Structure of the UI layer.
- [Whiteboard Construction](whiteboard_class_diagram.md) - Class diagram of the Whiteboard components.
- [Backend API Class Diagram](backend_api_class_diagram.md) - Interface between UI and CRDT backend.
- [Automerge Backend Class Diagram](automerge_backend_class_diagram.md) - Implementation of the CRDT backend.

## Behaviors
- [General App Flow](app_general_flow_sequence.md) - High-level overview of app lifecycle.
- [UI Drawing Sequence](ui_drawing_sequence.md) - Detailed flow of user interaction.
- [UI Drawing Sequence (PL)](ui_drawing_sequence_pl.md) - Detailed flow of drawing and rendering process in Polish.
- [Backend Local Intent Sequence](automerge_backend_local_intent_sequence.md) - Internal backend handling of local actions.
- [Backend Sync Sequence](automerge_backend_sync_sequence.md) - Internal backend handling of remote updates.
- [UI Connection Sequence](ui_connection_sequence.md) - detailed flow of connecting to the room and initializing channels.
- [UI Thread Communication Sequence](ui_thread_communication_sequence.md) - Detailed flow of the background network thread and LiveKit communication.
