# Correttore Date EXIF Foto

Strumento veloce per correggere le date EXIF delle foto basandosi sul nome del file e sui file JSON di Google Foto.

## Versione Rust (VELOCISSIMA! ⚡)

La versione Rust è **molto più veloce** della versione Python perché:
- Usa librerie native Rust per leggere EXIF (no chiamate esterne a exiftool)
- Parallelizza automaticamente con rayon (usa tutti i core della CPU)
- È compilata, quindi molto più veloce di Python interpretato

### Installazione

1. Installa Rust (se non già installato):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

2. Compila il progetto:
```bash
cd /home/alberto/takeout_photo/CorreggiEXIF
./build.sh
```

Oppure manualmente:
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release
```

### Uso

```bash
./target/release/corrigi-exif <directory>
```

Esempio:
```bash
./target/release/corrigi-exif "/home/alberto/takeout_photo/Takeout/Google Foto/Miglior foto_ Natura"
```

### Strategie disponibili

- `nome_file_preferito` (default): Preferisci anno dal nome file, altrimenti usa JSON
- `nome_file`: Usa solo anno dal nome file
- `json`: Usa solo data dal JSON
- `json_preferito`: Preferisci JSON, altrimenti nome file
- `exif_attuale`: Mantieni EXIF attuale

## Versione Python (più lenta ma con GUI)

La versione Python ha una GUI ma è più lenta. Per usarla:

```bash
python3 corrigi_date_foto_gui.py
```

## Performance

- **Rust**: ~0.5-2 secondi per 170 foto (parallelo, librerie native)
- **Python ottimizzato**: ~10-30 secondi per 170 foto (multiprocessing, chiamate a exiftool)
- **Python originale**: ~60-120 secondi per 170 foto (sequenziale)

La versione Rust è **10-50x più veloce**!

