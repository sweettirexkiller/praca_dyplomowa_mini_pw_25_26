sequenceDiagram
    actor User
    participant AppView as AppView (Main Thread)
    participant ChannelCmd as Command Channel (MPSC)
    participant ChannelMsg as Message Channel (MPSC)
    participant StdThread as std::thread
    participant TokioRuntime as Tokio Runtime
    participant LiveKit as LiveKit SDK

    User->>AppView: Click "Connect"
    activate AppView

    Note over AppView: 1. Initialize Channels
    AppView->>ChannelCmd: Create (tx_cmd, rx_cmd)
    Note right of ChannelCmd: For sending commands from UI -> Background
    AppView->>ChannelMsg: Create (tx_msg, rx_msg)
    Note right of ChannelMsg: For sending events from Background -> UI

    Note over AppView: 2. Store Channel Ends
    AppView->>AppView: livekit_command_sender = Some(tx_cmd)
    AppView->>AppView: app_msg_receiver = Some(rx_msg)

    Note over AppView: 3. Spawn Background Thread
    AppView->>StdThread: spawn(move || { ... })
    activate StdThread
    
    AppView-->>User: Set status to "Connecting..."
    deactivate AppView

    Note over StdThread: 4. Initialize Async Runtime
    StdThread->>TokioRuntime: Runtime::new()
    activate TokioRuntime
    
    Note over StdThread: 5. Connect to Room
    StdThread->>TokioRuntime: block_on(async { ... })
    TokioRuntime->>LiveKit: Room::connect(url, token)
    activate LiveKit
    LiveKit-->>TokioRuntime: (room, room_events)
    deactivate LiveKit

    Note over StdThread: 6. Enter Event Loop
    StdThread->>TokioRuntime: Start tokio::select! loop
    
    Note right of TokioRuntime: Listening on rx_cmd & room_events
    
    deactivate TokioRuntime
    deactivate StdThread
