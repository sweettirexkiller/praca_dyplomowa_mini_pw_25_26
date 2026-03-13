# RoomEvent Handling Diagram

How the background thread maps LiveKit `RoomEvent` variants to application messages (`AppMsg`).

```mermaid
classDiagram
    class RoomEvent {
        <<enumeration>>
        <<external - LiveKit SDK>>
        DataReceived(payload: Vec~u8~, participant: Option~RemoteParticipant~)
        ParticipantConnected(RemoteParticipant)
        ParticipantDisconnected(RemoteParticipant)
        Disconnected(reason: DisconnectReason)
    }

    class AppMsg {
        <<enumeration>>
        Log(String)
        ParticipantConnected(String)
        ParticipantDisconnected(String)
        NetworkMessage(sender: String, message: NetworkMessage)
    }

    class TransportPacket {
        <<enumeration>>
        Message(Vec~u8~)
        Chunk(id: u64, index: u32, total: u32, data: Vec~u8~)
    }

    class NetworkMessage {
        <<enumeration>>
        Sync(Vec~u8~)
        Chat(String)
        Cursor(x: i32, y: i32)
    }
```
