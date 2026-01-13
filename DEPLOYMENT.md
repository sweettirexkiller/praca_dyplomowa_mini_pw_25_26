# Dokumentacja Wdrożeniowa (Deployment Guide)

**Projekt:** Real-time Collaborative Whiteboard  
**Technologia:** Rust, LinkKit, Automerge, egui  

## 1. Wymagania Systemowe

Aby uruchomić aplikację w trybie deweloperskim lub produkcyjnym, wymagane są:

*   **System Operacyjny:** macOS (zalecane ze względu na biblioteki graficzne Metal) lub Linux/Windows (wymaga innej konfiguracji flag kompilatora).
*   **Rust Toolchain:** `stable` (zainstalowany przez `rustup`).
*   **Docker:** Do uruchomienia lokalnego serwera LiveKit.

## 2. Konfiguracja Serwera (LiveKit)

Aplikacja wymaga działającego serwera SFU (Selective Forwarding Unit) zgodnego z API LiveKit. Najprostszym sposobem jest uruchomienie oficjalnego obrazu Docker.

### Uruchomienie lokalne (Docker)

```bash
docker run -d \
    -p 7880:7880 \
    -p 7881:7881/udp \
    -e "LIVEKIT_KEYS=devkey:devsecret" \
    -e "LIVEKIT_LOG_LEVEL=info" \
    --name livekit-server \
    livekit/livekit-server \
    --node-ip 127.0.0.1 \
    --dev
```
*   API URL: `ws://127.0.0.1:7880`
*   API Key: `devkey`
*   API Secret: `devsecret`

## 3. Konfiguracja Klienta (Zmienne Środowiskowe)

Aplikacja kliencka (edytor) odczytuje konfigurację z pliku `.env` w katalogu głównym projektu (`editor/.env`) lub bezpośrednio ze zmiennych środowiskowych systemu.

Przykładowy plik `.env`:
```ini
LIVEKIT_API_KEY=devkey
LIVEKIT_API_SECRET=devsecret
LIVEKIT_URL=ws://127.0.0.1:7880
RUST_LOG=info
```

## 4. Budowanie i Uruchamianie

Ze względu na interakcję z systemem okienkowym macOS (Cocoa/Objective-C), konieczne jest przekazanie specyficznych flag linkera.

### Tryb Deweloperski (Debug)
Uruchomienie z terminala w katalogu `editor/`:

```bash
RUSTFLAGS="-C link-arg=-ObjC" cargo run
```

### Budowanie Wersji Release (Produkcyjnej)

Aby stworzyć zoptymalizowany plik wykonywalny:

```bash
RUSTFLAGS="-C link-arg=-ObjC" cargo build --release
```

Wynikowy plik binarny znajdzie się w:
`target/release/mac_textpad`

Można go uruchomić bezpośrednio, o ile plik `.env` znajduje się w tym samym katalogu, lub zmienne są ustawione w terminalu.

## 5. Dystrybucja

W obecnej wersji aplikacja jest dystrybuowana jako plik binarny. Aby przenieść ją na inny komputer (macOS):
1. Skopiuj plik `mac_textpad` (z folderu `target/release`).
2. Upewnij się, że użytkownik ma dostęp do serwera LiveKit (zmienna `LIVEKIT_URL`).
3. Uruchom przez terminal.

*(Opcjonalnie można spakować aplikację do `.app bundle` używając narzędzia `cargo-bundle`, co nie jest częścią standardowego procesu budowania Cargo).*
