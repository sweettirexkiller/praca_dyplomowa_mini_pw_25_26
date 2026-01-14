# Coolaborative Text Editor

Praca inzynierska Piotr Jankiewicz 

Mini PW Informatyka i Systemy informatyczne, 2025/26

Promotor: dr. inz. Paweł Kotowski

## Edytor

Projekt kolaboratywnego whiteboard umozliwia: 
- utworzenie pustego dokumentu
- zaladowanie gotowego dokumentu .png / .crdt 
- rysowania i scieranie tablicy 
- zmiane koloru i wielkosci pęzla
- utworzenie sesji kolaboracji 
- dołaczenie do sesji kolaboracji 
- odłaczenie się od sesji kolaboracji
- pokazywanie kursowa kooperanta edycji 
- zmiany kooperanta w czasie rzeczywistym

## Struktura folderów

- editor: głowny element projektu, czyli edytor obrazu

## Architekrura

Do działania projektu potrzebny jest uruchomiony serwer SFU od LiveKit. LiveKit 
to opensource serwer wykorzystywany do komunikacji real time z mozliwościa tworzenia
 pokoi/kanałów. Do odpalenia edytora potrzebne jest stworzenie pliku .env z odpowiednimi 
 stałymi zapewniającymi komunikację z serwerem. Struktura .env powinna być taka: 

 ```
LIVEKIT_API_KEY=key
LIVEKIT_API_SECRET=secret
LIVEKIT_URL=ipaddress:port
 ```

dla lokalnego serwera wygląda to tak: 

```
LIVEKIT_API_KEY=devkey
LIVEKIT_API_SECRET=devsecret
LIVEKIT_URL=127.0.0.1:7880
```

## Kompilacje i uruchomienie

Aby uruchomić edytor trzeba skorzystac z flagi przy uruchamianiu kompilatora by 
uzyc odpowiedniego kompliatora obiektowego c.

```
~ cargo run
```

### Jak uruchomić lokalny serwer SFU live kit i jak z niego korzystać w command line?

Aby uruchomić lokalny serwer trzeba mieć zainstalowany docker.

```
~ docker run -d \
-p 7880:7880 \
-p 7881:7881/udp \
-e "LIVEKIT_KEYS=devkey: devsecret" \
-e LIVEKIT_LOG_LEVEL=debug \
livekit/livekit-server \
--node-ip 127.0.0.1 \
--dev
~
```

W ten sposób uruchamiany jest serwer `livekit/livekit-server` który na naszej maszynie 
pod adresem 127.0.0.1:7800 ma uruchomione swoje API. Aby potwierdzić, ze serwer jest
 uruchomiony i responsywny mozna wykonać zapytanie: 

 ```
 ~ curl http://127.0.0.1:7880
 OK%
 ~
 ```

Ponaddto mozna podejrzec logi maszyny w nastepujacy sposob: 

```
~ docker ps 
CONTAINER ID   IMAGE                    COMMAND                  ...
abcd1234   livekit/livekit-server   "/livekit-server --n…"   ....
~  docker logs abcd1234

...LOGI...

```

#### Jak korzystać z CLI LiveKIT ?

Najpierw trzeba mieć ten command line zaintalowany. Na stacji roboczej macos 
mozna zainstalowac go w następujący sposób:

```
~ brew install livekit-cli
```

Aby wskazać projekt/serwer z którym command line móglby się łaczyć nalezy wykonać 
takie komendy: 

```
~  lk project add --api-key <key> --api-secret <secret> <project_name>
```

czyli dla połaczenia lokalnego nalezy wykonac: 

```
~ lk project add --api-key devkey --api-secret devsecret inzynierka_local
```

Wynikiem tej koncifguracji CLI jest: 

```
Saved CLI config to [/Users/-----/.livekit/cli-config.yaml]
┌────────────────────┬─────────────────────────────────────────┬─────────────────┐
│ Name               │ URL                                     │ API Key         │
├────────────────────┼─────────────────────────────────────────┼─────────────────┤
│   inzynierka_local │ ws://127.0.0.1:7880                     │ devkey          │
└────────────────────┴─────────────────────────────────────────┴─────────────────┘
```

Co nam to daje ? Najwazniejsze mozliwosci to podglad utworzonych kanałow i liczbe ich uzytkowników, ale CLI daje mozliwosc tworzenia nowych pokoi, access_tokenów do łączenia się z kanałami przez uzytkownikow. 

```
~ lk room list
Using project [local-inzynierka]
┌────────┬──────┬──────────────┬────────────┐
│ RoomID │ Name │ Participants │ Publishers │
├────────┼──────┼──────────────┼────────────┤
└────────┴──────┴──────────────┴────────────┘
```

Po więcej koment odsyłam do dokumentacji lib `lk --help`.

## Dokumentacja

Aby wygenerować dokumentację projektu z komentarzy w kodzie, należy skorzystać z narzędzia `cargo doc`. Poniższa komenda wygeneruje dokumentację i automatycznie otworzy ją w domyślnej przeglądarce:

```bash
cargo doc --no-deps --open
```

Opcja `--no-deps` powoduje wygenerowanie dokumentacji tylko dla kodu projektu (bez zależności zewnetrznych), co znacznie przyspiesza proces i czyni dokumentację bardziej czytelną.

c
## Przydatne komendy na windows jesli sa problemy z kompilacja 

```bash
winget install -e --id LLVM.LLVM --silent --accept-package-agreements --accept-source-agreements
```

dla weryfikacji instalacji nalezy sprawdzi czy clang-cl zostal dodany do PATH. 

```bash 
clang-cl --version
```
Jesli intalacja przebiegla pomyslnie ale program sie nie uruchomil nalezy dodac go do PATH: 

```bash 
$env:Path += ";C:\Program Files\LLVM\bin"
$env:CC="clang-cl"; $env:CXX="clang-cl"; clang-cl --version
```

Potem mozna juz uruchomic kompilacje i uruchomienie programu: 

```
~ cargo run
```

# UWAGA 

Jeśli program nadal się nie kompilje, bardzo mozliwe ze problem jest w starym msvc. Nalezy pobrac i zainstalowac najnowszy: 
[Visual Studio Latest Release](https://visualstudio.microsoft.com/downloads/) - [zrodlo rozwiazania](https://github.com/livekit/rust-sdks/issues/249)