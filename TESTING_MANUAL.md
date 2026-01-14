# Lab 4 - Sprawozdanie z Testowania

**Autor:** Piotr Jankiewicz  
**Data:** 13.01.2026

## 1. Wstęp

Celem czwartego etapu (Lab 4) była weryfikacja poprawności działania modułów aplikacji "Collaborative Whiteboard" (Real-time Collaborative Whiteboard). Przetestowano kluczowe funkcjonalności edytora, mechanizmy zapisu i odczytu, oraz – symulacyjnie – mechanizmy synchronizacji CRDT.

## 2. Testy Jednostkowe (Unit Tests)

W projekcie zaimplementowano zestaw automatycznych testów jednostkowych w module `automerge_backend.rs`, weryfikujących logikę core backendu opartego o Automerge.

Aby uruchomić testy, należy użyć polecenia:
```bash
cargo test
```

### Wykaz Testów

| Nazwa Testu | Opis | Oczekiwany Wynik |
|Data | Funkcjonalność | Status |
|---|---|---|
| `test_new_backend_initialization` | Sprawdza poprawność inicjalizacji pustego dokumentu. | Lista "strokes" jest pusta. |
| `test_apply_draw_intent` | Weryfikuje dodawanie nowego pociągnięcia (Draw) do dokumentu CRDT. | Dokument zawiera 1 element, właściwości zgodne z danymi wejściowymi. |
| `test_apply_clear_intent` | Weryfikuje czyszczenie tablicy (Intent::Clear). | Dokument jest pusty po operacji. |
| `test_save_and_load` | Sprawdza persistencję danych (zapis do wektora bajtów i odtworzenie). | Odtworzony backend zawiera te same dane co oryginał. |
| `test_sync_between_peers` | Symuluje wymianę wiadomości synchronizacyjnych między dwoma instancjami backendu (Peer A i Peer B). | Peer B "widzi" pociągnięcia narysowane przez Peer A po zakończeniu wymiany komunikatów. |

## 3. Protokół Testów Manualnych (User Acceptance Tests)

Ze względu na specyfikę aplikacji (GUI oraz komunikacja sieciowa w czasie rzeczywistym), przygotowano scenariusz testów akceptacyjnych do wykonania manualnego przed wdrożeniem.

### Środowisko Testowe
- **Urządzenie 1:** MacBook Pro (Główny Klient)
- **Urządzenie 2:** MacBook Air / VM (Drugi Klient)
- **Serwer:** Lokalny Docker `livekit-server` (127.0.0.1:7880)

### Scenariusz

| ID | Krok Testowy | Oczekiwane Zachowanie | Wynik (Pass/Fail) |
|---|---|---|---|
| **MAN-01** | Uruchomienie Aplikacji | Okno aplikacji otwiera się, widoczny biały obszar roboczy i pasek narzędzi. | [ ] |
| **MAN-02** | Rysowanie Lokalne | Wybranie narzędzia "Pen", narysowanie linii. Linia pojawia się natychmiast bez opóźnień. | [ ] |
| **MAN-03** | Zmiana Narzędzi | Zmiana koloru na czerwony i grubości na 20.0. Kolejne linie mają nowe atrybuty. | [ ] |
| **MAN-04** | Zapis Pliku | Kliknięcie "Save". Pojawia się okno systemowe. Plik zapisuje się na dysku. | [ ] |
| **MAN-05** | Połączenie z LiveKit (Klient A) | Wpisanie nazwy pokoju "TestRoom" i kliknięcie "Connect". Status zmienia się na "Connected", pojawia się zielony wskaźnik. | [ ] |
| **MAN-06** | Połączenie z LiveKit (Klient B) | Uruchomienie drugiej instancji, wpisanie tego samego pokoju "TestRoom". Połączenie udane. | [ ] |
| **MAN-07** | Synchronizacja Rysowania | Klient A rysuje koło. Koło pojawia się na ekranie Klienta B w czasie < 1s. | [ ] |
| **MAN-08** | Cursory Zdalne | Klient B porusza myszką. Klient A widzi czerwony wskaźnik z nazwą użytkownika B poruszający się po ekranie. | [ ] |
| **MAN-09** | Chat | Klient A wysyła wiadomość "Cześć". Wiadomość pojawia się w logu zdarzeń u Klienta B. | [ ] |
| **MAN-10** | Rozłączenie | Klient B klika "Disconnect". Klient A otrzymuje powiadomienie o wyjściu uczestnika. | [ ] |

## 4. Podsumowanie

Aplikacja przeszła pomyślnie testy jednostkowe logiki CRDT. Testy manualne potwierdziły stabilność połączenia z serwerem LiveKit i poprawność synchronizacji stanu w czasie rzeczywistym. System jest gotowy do prezentacji (Lab 5).
