# Drawing and Sync Flow Sequence Diagram

This diagram illustrates the flow of a local drawing action and its synchronization with remote peers, as well as receiving and applying remote drawing updates.

```mermaid
sequenceDiagram
    participant User
    participant UI as AppView (Main Thread)
    participant Backend as Automerge Backend
    participant Network as LiveKit Thread
    participant Peer as Remote Peer

    %% Local Drawing & Sync
    Note over User, Peer: Local Drawing & Sync
    User->>UI: Draw Stroke
    UI->>Backend: apply_intent(Intent::Draw)
    Backend-->>UI: FrontendUpdate (New Stroke)
    UI->>UI: Render Canvas

    UI->>Backend: generate_sync_message(PeerID)
    Backend-->>UI: Sync Message (Bytes)
    UI->>Network: AppCommand::Send(SyncMessage)
    Network->>Peer: Transmit Data

    %% Receiving Remote Updates
    Note over User, Peer: Receiving Remote Updates
    Peer->>Network: Transmit Data (SyncMessage)
    Network-->>UI: AppMsg::NetworkMessage(Sync)
    UI->>Backend: receive_sync_message(PeerID, Data)
    Backend->>Backend: Merge CRDT State
    Backend-->>UI: FrontendUpdate (Merged State)
    UI->>UI: Re-render Canvas (Show Peer's Stroke)
```
