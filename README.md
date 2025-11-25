# Correttore Date EXIF Foto

Strumento veloce per correggere le date EXIF delle foto basandosi sul nome del file e sui file JSON di Google Foto.

**Ottimizzato per Google Takeout**: Questo strumento è progettato specificamente per lavorare con le foto esportate da Google Foto tramite Google Takeout. Legge automaticamente i file JSON supplementari (`.supplemental-metadata.json`, `.supplemental.json`, ecc.) che Google Foto genera durante l'esportazione per recuperare le date originali delle foto.

## Versione Rust (VELOCISSIMA! ⚡)

La versione Rust è **molto più veloce** della versione Python perché:
- Usa librerie native Rust per leggere EXIF (no chiamate esterne a exiftool)
- Parallelizza automaticamente con rayon (usa tutti i core della CPU)
- È compilata, quindi molto più veloce di Python interpretato
- Include GUI moderna con egui

### Installazione

1. Installa Rust (se non già installato):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

Oppure aggiungi questa riga al tuo `~/.bashrc` per caricarlo automaticamente:
```bash
echo 'source $HOME/.cargo/env' >> ~/.bashrc
source ~/.bashrc
```

2. Verifica che Rust sia disponibile:
```bash
cargo --version
```

Dovresti vedere qualcosa come: `cargo 1.91.1 (ed61e7d7e 2025-11-07)`

3. Rendi eseguibile lo script di build (solo la prima volta):
```bash
cd /home/alberto/takeout_photo/CorreggiEXIF
chmod +x ./build.sh
```

4. Compila il progetto:

**Opzione A: Usa lo script di build (consigliato)**
```bash
cd /home/alberto/takeout_photo/CorreggiEXIF
source $HOME/.cargo/env
./build.sh
```

**Opzione B: Compila manualmente**
```bash
cd /home/alberto/takeout_photo/CorreggiEXIF
source $HOME/.cargo/env
cargo build --release
```

La prima compilazione può richiedere alcuni minuti perché scarica e compila tutte le dipendenze. Le compilazioni successive saranno molto più veloci.

### Uso

#### GUI (Interfaccia Grafica)

Per avviare la GUI, esegui senza argomenti:
```bash
./target/release/corrigi-exif
```

La GUI include:
- Tabella con colonne: Nome File, DateTimeOriginal ⭐, CreateDate, ModifyDate, Strategia
- Pannello laterale con le 3 fasi:
  - **Fase 1**: Selezione cartella
  - **Fase 2**: Proposta modifiche (strategia globale, calcola proposte)
  - **Fase 3**: Applica modifiche
- Evidenziazione delle righe con proposte (giallo)
- Statistiche in tempo reale

#### CLI (Riga di Comando)

Per usare la versione CLI, passa la directory come argomento:
```bash
./target/release/corrigi-exif <directory>
```

**Esempio:**
```bash
./target/release/corrigi-exif "/home/alberto/takeout_photo/Takeout/Google Foto/Miglior foto_ Natura"
```

### Strategie disponibili

- `nome_file_preferito` (default): Preferisci anno dal nome file, altrimenti usa JSON
- `nome_file`: Usa solo anno dal nome file
- `json`: Usa solo data dal JSON
- `json_preferito`: Preferisci JSON, altrimenti nome file
- `exif_attuale`: Mantieni EXIF attuale

### Output atteso (CLI)

Il programma mostrerà:
- Quante foto ha trovato
- Tempo impiegato per la lettura (dovrebbe essere velocissimo!)
- Statistiche (totale, con EXIF, con proposte)
- Prime 10 foto con proposte di modifica

### Troubleshooting

#### Errore: "command not found: cargo"
```bash
source $HOME/.cargo/env
export PATH="$HOME/.cargo/bin:$PATH"
```

#### Errore durante la compilazione

**Errore: "failed to select a version for the requirement `exif = \"^0.8\"" o `exif = \"^0.7\""`
```bash
# Questo errore significa che la libreria `exif` non esiste con quelle versioni
# Il progetto usa `kamadak-exif` invece di `exif`
# Se vedi questo errore, verifica che Cargo.toml usi:
#   exif = { package = "kamadak-exif", version = "0.6" }
# invece di:
#   exif = "0.7" o "0.8"
cd /home/alberto/takeout_photo/CorreggiEXIF
cargo update
cargo build --release
```

**Altri errori di compilazione:**
- Assicurati che tutte le dipendenze siano installate. Rust le scaricherà automaticamente durante la prima compilazione.
- Se vedi errori di versione, prova: `cargo update` per aggiornare le dipendenze

**Warning: "the following packages contain code that will be rejected by a future version of Rust: ashpd"**
- Questo è un warning di compatibilità futura da una dipendenza transitiva (`ashpd`, usata da `rfd` per il file dialog)
- Non è un errore critico e il programma funziona correttamente
- Il warning verrà risolto quando gli autori delle librerie aggiorneranno le loro dipendenze
- Puoi ignorarlo tranquillamente

### Performance

La versione Rust è **10-50x più veloce** della versione Python:
- **Rust**: ~0.5-2 secondi per 170 foto (parallelo, librerie native)
- **Python ottimizzato**: ~10-30 secondi per 170 foto (multiprocessing, chiamate a exiftool)
- **Python originale**: ~60-120 secondi per 170 foto (sequenziale)

## Versione Python (deprecata)

La versione Python è stata rimossa in favore della versione Rust che include anche la GUI ed è molto più veloce.

## Licenza

Questo progetto è rilasciato sotto licenza MIT.
