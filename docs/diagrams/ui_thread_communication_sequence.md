sequenceDiagram
    actor User
    participant AppView as AppView (Main Thread)
    participant Channel as Channels (MPSC)
    participant Thread as Background Thread
    participant Tokio as Tokio Runtime
    participant LiveKit as LiveKit SDK (Room)

    User->>AppView: Click "Connect"
    activate AppView
    
    Note over AppView: Generate Token
    AppView->>Channel: Create (tx_cmd, rx_cmd)
    AppView->>Channel: Create (tx_msg, rx_msg)
    
    AppView->>Thread: spawn(move || { ... })
    activate Thread
    
    AppView-->>User: Update Status "Connecting..."
    deactivate AppView

    Thread->>Tokio: Runtime::new()
    activate Tokio
    
    Tokio->>Tokio: block_on(async { ... })
    
    Tokio->>LiveKit: Room::connect(url, token)
    activate LiveKit
    LiveKit-->>Tokio: (room, room_events)
    deactivate LiveKit
    
    loop Event Loop (tokio::select!)
        par Handle Outgoing Commands
            AppView->>Channel: Send AppCommand (Broadcast/Send)
            Channel->>Tokio: rx_cmd.recv()
            
            alt Message > 14KB
                Tokio->>Tokio: Chunk Message
                loop For Each Chunk
                    Tokio->>LiveKit: publish_data(chunk)
                end
            else Small Message
                Tokio->>LiveKit: publish_data(message)
            end
            
        and Handle Incoming Events
            LiveKit->>Tokio: room_events.recv() (DataReceived)
            
            alt Is Chunk
                Tokio->>Tokio: Buffer & Reassemble
            end
            
            opt On Complete Message
                Tokio->>Channel: tx_msg.send(AppMsg::NetworkMessage)
                Channel->>AppView: Queue Message
                Tokio->>AppView: ctx.request_repaint()
            end
        end
    end
    
    deactivate Tokio
    deactivate Thread
    
    Note over AppView: On Next Frame (update)
    AppView->>Channel: Check app_msg_receiver
    Channel->>AppView: Process AppMsg (Sync/Chat/Cursor)
