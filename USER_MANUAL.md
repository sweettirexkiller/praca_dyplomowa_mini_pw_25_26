# Instrukcja Użytkownika (User Manual)

**Aplikacja:** Mac TextPad (Collaborative Whiteboard)  
**Wersja:** 1.0  

## 1. Wprowadzenie

Aplikacja służy do wspólnego rysowania i pracy na wirtualnej tablicy w czasie rzeczywistym. Pozwala użytkownikom łączyć się w "Pokoje" (Rooms), gdzie każdy uczestnik widzi zmiany wprowadzane przez innych natychmiastowo.

## 2. Interfejs Użytkownika

Główne okno aplikacji składa się z trzech sekcji:

1.  **Górny Pasek (Toolbar):** Zawiera podstawowe narzędzia edycji i pliku.
2.  **Panel Boczny (Sidebar):** (Domyślnie ukryty, dostępny pod `Cmd + \` lub przyciskiem "Menu") Służy do zarządzania połączeniem sieciowym.
3.  **Obszar Roboczy (Canvas):** Centralne miejsce do rysowania.

## 3. Podstawowe Funkcje

### Rysowanie
*   **Narzędzia:** Domyślnie wybrane jest pióro (**Pen**). Możesz przełączyć na Gumkę (**Eraser**) w górnym pasku, aby usuwać fragmenty rysunku.
*   **Kolor:** Kliknij w pole koloru na pasku narzędzi, aby otworzyć próbnik i wybrać barwę pędzla.
*   **Rozmiar:** Użyj suwaka "Size", aby zmienić grubość linii.

### Zarządzanie Plikami
*   **Nowy:** Przycisk "New" czyści tablicę (wymaga potwierdzenia, jeśli są niezapisane zmiany).
*   **Zapis:** Przycisk "Save" (lub skrót `Cmd + S`) pozwala zapisać obecny stan tablicy.
*   **Otwórz:** Przycisk "Open" (lub skrót `Cmd + O`) ładuje wcześniej zapisany plik.

## 4. Praca Grupowa (LiveKit)

Aby pracować z innymi, musisz połączyć się z serwerem.

1.  Otwórz panel boczny przyciskiem **"☰ Menu"**.
2.  W sekcji "LiveKit":
    *   **Room:** Wpisz nazwę pokoju (np. "Projekt1"). Jeśli zostawisz puste, zostanie wygenerowana losowa nazwa.
    *   **Identity:** Twoja nazwa widoczna dla innych (np. "Jan").
3.  Kliknij **"Connect"**.
4.  Po połączeniu:
    *   Przycisk zmieni się na **"Disconnect"**.
    *   Na dole panelu zobaczysz listę uczestników ("Participants").
    *   Ruchy myszki innych osób będą widoczne jako kolorowe kropki z ich imionami.
    *   Wszystko co narysujesz, pojawi się u innych.

### Chat
Po połączeniu, w panelu bocznym dostępny jest prosty czat. Wpisz wiadomość w polu "Message" i kliknij "Send", aby wysłać ją do wszystkich w pokoju.

## 5. Skróty Klawiszowe

| Skrót | Akcja |
|---|---|
| `Cmd + S` | Zapisz plik |
| `Cmd + O` | Otwórz plik |
| `Cmd + \` | Pokaż/Ukryj Panel Boczny |

## 6. Rozwiązywanie Problemów

*   **Brak połączenia:** Upewnij się, że serwer LiveKit działa (zobacz dokumentację wdrożeniową) i zmienne środowiskowe (`LIVEKIT_URL`) są poprawne.
*   **Opóźnienia:** Przy słabym połączeniu internetowym synchronizacja może trwać do kilku sekund.
