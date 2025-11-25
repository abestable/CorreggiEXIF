#!/bin/bash
# Script per configurare Git e fare il commit iniziale

cd "$(dirname "$0")"

# Verifica se git è installato
if ! command -v git &> /dev/null; then
    echo "Errore: Git non è installato"
    echo "Installa con: sudo apt-get install git"
    exit 1
fi

# Inizializza repository se non esiste
if [ ! -d .git ]; then
    echo "Inizializzazione repository Git..."
    git init
fi

# Configura git se necessario
if [ -z "$(git config user.name)" ]; then
    echo "Configurazione Git..."
    read -p "Inserisci il tuo nome: " name
    git config user.name "$name"
fi

if [ -z "$(git config user.email)" ]; then
    read -p "Inserisci la tua email: " email
    git config user.email "$email"
fi

# Aggiungi tutti i file
echo "Aggiunta file al repository..."
git add .

# Mostra lo stato
echo ""
echo "File da committare:"
git status --short

# Chiedi conferma
read -p "Vuoi fare il commit? (y/n): " confirm
if [ "$confirm" = "y" ] || [ "$confirm" = "Y" ]; then
    git commit -m "Initial commit: Correttore Date EXIF Foto

- Versione Rust veloce con parallelizzazione
- Versione Python con GUI
- Supporto per correzione date EXIF basata su nome file e JSON Google Foto
- Strategie multiple per determinare la data corretta"
    
    echo ""
    echo "✅ Commit completato!"
    echo ""
    echo "Per vedere i log:"
    echo "  git log --oneline"
    echo ""
    echo "Per aggiungere un remote (es. GitHub):"
    echo "  git remote add origin <url>"
    echo "  git push -u origin main"
else
    echo "Commit annullato"
fi

