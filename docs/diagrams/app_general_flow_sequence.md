# General Application Flow Sequence Diagram

This diagram illustrates the high-level lifecycle of the application: initialization, connecting to a room, local user actions (drawing), and synchronization of events from remote peers.

```mermaid
sequenceDiagram
    participant User
    participant UI as AppView (Main Thread)
    participant Backend as Automerge Backend
    participant Network as LiveKit Thread
    participant Peer as Remote Peer

    %% Initialization Phase
    Note over User, Backend: Initialization Phase
    User->>UI: Launch Application
    UI->>Backend: new()
    Backend-->>UI: Initial State (Empty or Loaded)
    
    %% Connection Phase
    Note over User, Peer: Connection Phase
    User->>UI: Click "Join/Connect"
    UI->>Network: Connect(room, token) [Spawn Thread]
    Network->>Network: LiveKit Connect
    Network-->>UI: AppMsg::Connected
    Network-->>UI: AppMsg::ParticipantConnected(Peer)
    UI->>Backend: peer_connected(PeerID)
    
    %% Local Drawing Phase
    Note over User, Peer: Local Drawing & Sync
    User->>UI: Interact (Draw Stroke)
    UI->>Backend: apply_intent(Intent::Draw)
    Backend-->>UI: FrontendUpdate (New Stroke)
    UI->>UI: Render Canvas
    
    UI->>Backend: generate_sync_message(PeerID)
    Backend-->>UI: Sync Message (Bytes)
    UI->>Network: AppCommand::Send(SyncMessage)
    Network->>Peer: Transmit Data
    
    %% Remote Sync Phase
    Note over User, Peer: Receiving Remote Updates
    Peer->>Network: Transmit Data (SyncMessage)
    Network-->>UI: AppMsg::NetworkMessage(Sync)
    UI->>Backend: receive_sync_message(PeerID, Data)
    Backend->>Backend: Merge CRDT State
    Backend-->>UI: FrontendUpdate (Merged State)
    UI->>UI: Re-render Canvas (Show Peer's Stoke)
```
