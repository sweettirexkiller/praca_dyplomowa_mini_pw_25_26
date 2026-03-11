# UI Thread Communication Sequence

Detailed flow of messages between the UI thread and the background network thread during collaborative editing. Split into 6 parts:

1. [Local Drawing](thread_comm_local_drawing.md) — User draws a stroke, sync is sent to peers.
2. [Receiving Remote Data](thread_comm_receiving_data.md) — Incoming data from a peer is deserialized and applied.
3. [Participant Connects](thread_comm_participant_connects.md) — A remote peer joins and initial sync is exchanged.
4. [Participant Disconnects](thread_comm_participant_disconnects.md) — A remote peer leaves and state is cleaned up.
5. [Cursor Broadcasting](thread_comm_cursor_broadcasting.md) — Local cursor position is broadcast to all peers.
6. [Disconnect](thread_comm_disconnect.md) — Local user disconnects from the room.