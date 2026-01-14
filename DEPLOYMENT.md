# Dokumentacja Wdrożeniowa (Deployment Guide)

**Projekt:** Real-time Collaborative Whiteboard  
**Technologia:** Rust, LinkKit, Automerge, egui  

## 1. Uruchomienie Aplikacji w środowisku ALFA

- zdalny serwer (mozliwosc testowania pracy zdalnej)
- zbudowany gotowy plik do uruchomienia na macos i windows
- nalezy rozpokowac odpowiednia paczke .zip w folderze /releases/ i uruchomic w terminalu program


## 2. Uruchomienie Aplikacji w środowisku Deweloperskim
- nalezy zbudowac aplikację zgodnie z dokumentacją w README.md

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
```

## 4. Budowanie i Uruchamianie

W zależności od systemu operacyjnego, proces budowania może wymagać specyficznych flag lub komend.

### macOS

Ze względu na interakcję z systemem okienkowym macOS (Cocoa/Objective-C), konieczne jest przekazanie specyficznych flag linkera.

#### Tryb Deweloperski (Debug)
Uruchomienie z terminala w katalogu `editor/`:

```bash
RUSTFLAGS="-C link-arg=-ObjC" cargo run
```

#### Budowanie Wersji Release (Produkcyjnej)

Aby stworzyć zoptymalizowany plik wykonywalny:

```bash
RUSTFLAGS="-C link-arg=-ObjC" cargo build --release
```

Wynikowy plik binarny znajdzie się w:
`target/release/mac_textpad`

### Windows

Na systemie Windows standardowe komendy `cargo` są wystarczające.

#### Tryb Deweloperski (Debug)
Uruchomienie z terminala w katalogu `editor/`:

```powershell
cargo run
```

#### Budowanie Wersji Release (Produkcyjnej)

Aby stworzyć zoptymalizowany plik wykonywalny:

```powershell
cargo build --release
```

Wynikowy plik binarny (`collaboratite_editor.exe`) znajdzie się w:
`target\release\collaboratite_editor.exe`

### Uruchamianie

Można uruchomić zbudowany plik bezpośrednio, o ile plik `.env` znajduje się w tym samym katalogu, lub zmienne środowiskowe są ustawione w terminalu.

## 5. Dystrybucja

W obecnej wersji aplikacja jest dystrybuowana jako plik binarny. Aby przenieść ją na inny komputer:

**macOS:**
1. Skopiuj plik `collaboratite_editor` (z folderu `target/release`).
2. Upewnij się, że użytkownik ma dostęp do serwera LiveKit.
3. Uruchom przez terminal.
*(Opcjonalnie można spakować aplikację do `.app bundle` używając narzędzia `cargo-bundle`, przy czym nazwa binarki to `collaboratite_editor`).*

**Windows:**
1. Skopiuj plik `collaboratite_editor.exe` (z folderu `target\release`).
2. Upewnij się, że użytkownik ma dostęp do serwera LiveKit.
3. Uruchom dwuklikiem lub przez PowerShell/CMD.
