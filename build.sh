#!/bin/bash
# Script per compilare il progetto Rust

export PATH="$HOME/.cargo/bin:$PATH"

if ! command -v cargo &> /dev/null; then
    echo "Errore: Cargo non trovato. Assicurati che Rust sia installato."
    echo "Esegui: source \$HOME/.cargo/env"
    exit 1
fi

echo "Compilazione in corso..."
cargo build --release

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Compilazione completata!"
    echo "Eseguibile disponibile in: target/release/corrigi-exif"
    echo ""
    echo "Per eseguire:"
    echo "  ./target/release/corrigi-exif <directory>"
else
    echo "❌ Errore durante la compilazione"
    exit 1
fi

