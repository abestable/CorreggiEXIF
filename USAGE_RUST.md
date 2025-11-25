# Come usare la versione Rust

## Passo 1: Carica Rust nel PATH

Rust è stato installato ma potrebbe non essere nel PATH della sessione corrente. Esegui:

```bash
source $HOME/.cargo/env
```

Oppure aggiungi questa riga al tuo `~/.bashrc` per caricarlo automaticamente:
```bash
echo 'source $HOME/.cargo/env' >> ~/.bashrc
source ~/.bashrc
```

## Passo 2: Verifica che Rust sia disponibile

```bash
cargo --version
```

Dovresti vedere qualcosa come: `cargo 1.91.1 (ed61e7d7e 2025-11-07)`

## Passo 3: Rendi eseguibile lo script di build (solo la prima volta)

```bash
cd /home/alberto/takeout_photo/CorreggiEXIF
chmod +x ./build.sh
```

## Passo 4: Compila il progetto

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

## Passo 5: Esegui il programma

```bash
./target/release/corrigi-exif "/path/to/tua/cartella"
```

**Esempio:**
```bash
./target/release/corrigi-exif "/home/alberto/takeout_photo/Takeout/Google Foto/MigliorfotoNatura"
```

## Output atteso

Il programma mostrerà:
- Quante foto ha trovato
- Tempo impiegato per la lettura (dovrebbe essere velocissimo!)
- Statistiche (totale, con EXIF, con proposte)
- Prime 10 foto con proposte di modifica

## Troubleshooting

### Errore: "command not found: cargo"
```bash
source $HOME/.cargo/env
export PATH="$HOME/.cargo/bin:$PATH"
```

