#!/usr/bin/env python3
"""
GUI per correggere le date EXIF delle foto basandosi sul nome del file e JSON.
"""

import json
import os
import re
import subprocess
import tkinter as tk
from concurrent.futures import ProcessPoolExecutor, as_completed
from datetime import datetime
from pathlib import Path
from tkinter import ttk, filedialog, messagebox
from typing import Dict, List, Optional, Tuple
import multiprocessing

# Strategie disponibili per determinare la data
STRATEGIE = {
    'nome_file': 'Usa anno dal nome file',
    'json': 'Usa data dal JSON',
    'exif_attuale': 'Mantieni EXIF attuale',
    'nome_file_preferito': 'Preferisci nome file, altrimenti JSON',
    'json_preferito': 'Preferisci JSON, altrimenti nome file'
}

class FotoData:
    """Classe per memorizzare i dati di una foto."""
    def __init__(self, path: Path):
        self.path = path
        self.nome_file = path.name
        self.anno_nome = None
        self.data_nome = None  # (anno, mese, giorno) o None
        self.data_json = None
        self.campi_exif = {
            'DateTimeOriginal': None,
            'CreateDate': None,
            'ModifyDate': None
        }
        self.proposte = {
            'DateTimeOriginal': None,
            'CreateDate': None,
            'ModifyDate': None
        }
        self.strategia = 'nome_file_preferito'  # Strategia di default
        self.json_path = None

def estrai_anno_da_nome(nome_file: str) -> Optional[Tuple[int, int, int]]:
    """Estrae l'anno/data dal nome del file."""
    # Pattern: "2002_" all'inizio
    match = re.search(r'^(\d{4})_', nome_file)
    if match:
        anno = int(match.group(1))
        if 1900 <= anno <= 2100:
            return (anno, 1, 1)
    
    # Pattern: "20050806" (YYYYMMDD)
    match = re.search(r'(\d{4})(\d{2})(\d{2})', nome_file)
    if match:
        anno = int(match.group(1))
        mese = int(match.group(2))
        giorno = int(match.group(3))
        if 1900 <= anno <= 2100 and 1 <= mese <= 12 and 1 <= giorno <= 31:
            return (anno, mese, giorno)
    
    # Pattern: "24082009" (DDMMYYYY)
    match = re.search(r'(\d{2})(\d{2})(\d{4})', nome_file)
    if match:
        giorno = int(match.group(1))
        mese = int(match.group(2))
        anno = int(match.group(3))
        if 1900 <= anno <= 2100 and 1 <= mese <= 12 and 1 <= giorno <= 31:
            return (anno, mese, giorno)
    
    return None

def trova_file_json(foto_path: Path) -> Optional[Path]:
    """Trova il file JSON corrispondente alla foto."""
    base_name = foto_path.stem
    directory = foto_path.parent
    
    possibili_nomi = [
        f"{foto_path.name}.supplemental-metadata.json",
        f"{base_name}.supplemental-metadata.json",
        f"{foto_path.name}.supplemental.json",
        f"{base_name}.supplemental.json",
        f"{foto_path.name}.supplemental-met.json",
        f"{base_name}.supplemental-met.json",
        f"{foto_path.name}.supplemental-me.json",
        f"{base_name}.supplemental-me.json",
        f"{foto_path.name}.supplemental-metad.json",
        f"{base_name}.supplemental-metad.json",
        f"{foto_path.name}.supplementa.json",
        f"{base_name}.supplementa.json",
        f"{foto_path.name}.supplemen.json",
        f"{base_name}.supplemen.json",
    ]
    
    for nome_json in possibili_nomi:
        json_path = directory / nome_json
        if json_path.exists():
            return json_path
    
    return None

def leggi_data_json(json_path: Path) -> Optional[datetime]:
    """Legge la data dal file JSON."""
    try:
        with open(json_path, 'r', encoding='utf-8') as f:
            data = json.load(f)
        
        if 'photoTakenTime' in data and 'timestamp' in data['photoTakenTime']:
            timestamp = int(data['photoTakenTime']['timestamp'])
            return datetime.fromtimestamp(timestamp)
        
        return None
    except:
        return None

def ottieni_tutti_campi_exif(foto_path: Path) -> Dict[str, Optional[datetime]]:
    """Ottiene tutti i campi EXIF importanti dalla foto (ottimizzato: una sola chiamata a exiftool)."""
    campi = {
        'DateTimeOriginal': None,
        'CreateDate': None,
        'ModifyDate': None
    }
    
    try:
        # Chiama exiftool una sola volta per tutti i campi
        result = subprocess.run(
            ['exiftool', '-DateTimeOriginal', '-CreateDate', '-ModifyDate', '-s3', '-S', str(foto_path)],
            capture_output=True,
            text=True,
            check=False
        )
        
        if result.returncode == 0:
            lines = result.stdout.strip().split('\n')
            for line in lines:
                line = line.strip()
                if not line:
                    continue
                
                # Parse formato: "DateTimeOriginal: 2002:01:01 12:00:00"
                parts = line.split(':', 1)
                if len(parts) == 2:
                    campo_nome = parts[0].strip()
                    valore = parts[1].strip()
                    
                    if valore and ':' in valore and ' ' in valore:
                        try:
                            dt = datetime.strptime(valore.split('+')[0].split('-')[0], '%Y:%m:%d %H:%M:%S')
                            if campo_nome == 'DateTimeOriginal':
                                campi['DateTimeOriginal'] = dt
                            elif campo_nome == 'CreateDate':
                                campi['CreateDate'] = dt
                            elif campo_nome == 'ModifyDate':
                                campi['ModifyDate'] = dt
                        except ValueError:
                            pass
    except Exception as e:
        print(f"[DEBUG] Errore leggendo EXIF di {foto_path.name}: {e}")
    
    return campi

def calcola_proposta(foto: FotoData) -> Dict[str, Optional[datetime]]:
    """Calcola le proposte di modifica basate sulla strategia."""
    proposte = {
        'DateTimeOriginal': None,
        'CreateDate': None,
        'ModifyDate': None
    }
    
    strategia = foto.strategia
    
    # Determina quale data usare
    data_da_usare = None
    
    if strategia == 'nome_file':
        if foto.data_nome:
            anno, mese, giorno = foto.data_nome
            data_da_usare = datetime(anno, mese, giorno, 12, 0, 0)
    elif strategia == 'json':
        if foto.data_json:
            data_da_usare = foto.data_json
    elif strategia == 'exif_attuale':
        data_da_usare = foto.campi_exif.get('DateTimeOriginal')
    elif strategia == 'nome_file_preferito':
        if foto.data_nome:
            anno, mese, giorno = foto.data_nome
            data_da_usare = datetime(anno, mese, giorno, 12, 0, 0)
        elif foto.data_json:
            data_da_usare = foto.data_json
    elif strategia == 'json_preferito':
        if foto.data_json:
            data_da_usare = foto.data_json
        elif foto.data_nome:
            anno, mese, giorno = foto.data_nome
            data_da_usare = datetime(anno, mese, giorno, 12, 0, 0)
    
    if data_da_usare:
        # Proponi di modificare solo DateTimeOriginal (quello che usa Immich)
        proposte['DateTimeOriginal'] = data_da_usare
    
    return proposte

def leggi_foto_singola(foto_path: Path) -> FotoData:
    """Legge i dati di una singola foto (usata per multiprocessing)."""
    foto = FotoData(foto_path)
    
    # Estrai anno dal nome
    foto.data_nome = estrai_anno_da_nome(foto.nome_file)
    if foto.data_nome:
        foto.anno_nome = foto.data_nome[0]
    
    # Leggi JSON
    foto.json_path = trova_file_json(foto_path)
    if foto.json_path:
        foto.data_json = leggi_data_json(foto.json_path)
    
    # Leggi EXIF
    foto.campi_exif = ottieni_tutti_campi_exif(foto_path)
    
    return foto

def imposta_data_exif(foto_path: Path, data: datetime, solo_datetime_original: bool = True) -> bool:
    """Imposta la data EXIF nella foto."""
    data_str = f"{data.year:04d}:{data.month:02d}:{data.day:02d} {data.hour:02d}:{data.minute:02d}:00"
    
    try:
        comando = ['exiftool', '-overwrite_original']
        
        if solo_datetime_original:
            comando.append(f'-DateTimeOriginal={data_str}')
        else:
            comando.extend([
                f'-DateTimeOriginal={data_str}',
                f'-CreateDate={data_str}',
                f'-ModifyDate={data_str}'
            ])
        
        comando.append(str(foto_path))
        
        subprocess.run(comando, capture_output=True, check=True)
        return True
    except:
        return False

class FotoCorrectorGUI:
    def __init__(self, root):
        self.root = root
        self.root.title("Correttore Date EXIF Foto")
        self.root.geometry("1400x800")
        
        self.foto_list: List[FotoData] = []
        self.directory_selezionata = None
        
        self.setup_ui()
    
    def setup_ui(self):
        """Crea l'interfaccia utente."""
        # Frame principale
        main_frame = ttk.Frame(self.root, padding="10")
        main_frame.grid(row=0, column=0, sticky=(tk.W, tk.E, tk.N, tk.S))
        
        self.root.columnconfigure(0, weight=1)
        self.root.rowconfigure(0, weight=1)
        main_frame.columnconfigure(0, weight=1)
        main_frame.rowconfigure(1, weight=1)
        
        # Fase 1: Selezione cartella
        fase1_frame = ttk.LabelFrame(main_frame, text="Fase 1: Leggi Cartella", padding="10")
        fase1_frame.grid(row=0, column=0, sticky=(tk.W, tk.E), pady=(0, 10))
        
        ttk.Button(fase1_frame, text="Seleziona Cartella", command=self.seleziona_cartella).pack(side=tk.LEFT, padx=5)
        self.cartella_label = ttk.Label(fase1_frame, text="Nessuna cartella selezionata")
        self.cartella_label.pack(side=tk.LEFT, padx=10)
        
        # Tabella foto
        table_frame = ttk.Frame(main_frame)
        table_frame.grid(row=1, column=0, sticky=(tk.W, tk.E, tk.N, tk.S))
        table_frame.columnconfigure(0, weight=1)
        table_frame.rowconfigure(0, weight=1)
        
        # Scrollbar
        scrollbar_y = ttk.Scrollbar(table_frame, orient=tk.VERTICAL)
        scrollbar_y.grid(row=0, column=1, sticky=(tk.N, tk.S))
        
        scrollbar_x = ttk.Scrollbar(table_frame, orient=tk.HORIZONTAL)
        scrollbar_x.grid(row=1, column=0, sticky=(tk.W, tk.E))
        
        # Treeview (tabella)
        columns = ('Nome File', 'DateTimeOriginal', 'CreateDate', 'ModifyDate', 'Strategia')
        self.tree = ttk.Treeview(table_frame, columns=columns, show='headings', 
                                  yscrollcommand=scrollbar_y.set, xscrollcommand=scrollbar_x.set)
        
        scrollbar_y.config(command=self.tree.yview)
        scrollbar_x.config(command=self.tree.xview)
        
        # Configura colonne
        self.tree.heading('Nome File', text='Nome File')
        self.tree.heading('DateTimeOriginal', text='DateTimeOriginal ⭐')
        self.tree.heading('CreateDate', text='CreateDate')
        self.tree.heading('ModifyDate', text='ModifyDate')
        self.tree.heading('Strategia', text='Strategia')
        
        self.tree.column('Nome File', width=300)
        self.tree.column('DateTimeOriginal', width=200)
        self.tree.column('CreateDate', width=200)
        self.tree.column('ModifyDate', width=200)
        self.tree.column('Strategia', width=250)
        
        self.tree.grid(row=0, column=0, sticky=(tk.W, tk.E, tk.N, tk.S))
        
        # Bind per modificare strategia
        self.tree.bind('<Double-1>', self.on_double_click)
        
        # Pannello laterale destro
        side_panel = ttk.LabelFrame(main_frame, text="Controlli", padding="10")
        side_panel.grid(row=0, column=1, rowspan=2, sticky=(tk.W, tk.E, tk.N, tk.S), padx=(10, 0))
        
        # Fase 2: Proposta modifiche
        fase2_frame = ttk.LabelFrame(side_panel, text="Fase 2: Proposta Modifiche", padding="10")
        fase2_frame.pack(fill=tk.X, pady=(0, 10))
        
        ttk.Label(fase2_frame, text="Strategia globale:").pack(anchor=tk.W)
        self.strategia_globale = ttk.Combobox(fase2_frame, values=list(STRATEGIE.values()), 
                                               state='readonly', width=30)
        self.strategia_globale.set(STRATEGIE['nome_file_preferito'])
        self.strategia_globale.pack(fill=tk.X, pady=5)
        self.strategia_globale.bind('<<ComboboxSelected>>', self.cambia_strategia_globale)
        
        ttk.Button(fase2_frame, text="Calcola Proposte", command=self.calcola_proposte).pack(fill=tk.X, pady=5)
        
        # Fase 3: Applica modifiche
        fase3_frame = ttk.LabelFrame(side_panel, text="Fase 3: Applica Modifiche", padding="10")
        fase3_frame.pack(fill=tk.X)
        
        ttk.Label(fase3_frame, text="Opzioni:").pack(anchor=tk.W)
        self.solo_datetime = tk.BooleanVar(value=True)
        ttk.Checkbutton(fase3_frame, text="Solo DateTimeOriginal (consigliato)", 
                       variable=self.solo_datetime).pack(anchor=tk.W)
        
        ttk.Button(fase3_frame, text="Applica Modifiche", command=self.applica_modifiche).pack(fill=tk.X, pady=10)
        
        # Statistiche
        self.stats_label = ttk.Label(side_panel, text="")
        self.stats_label.pack(pady=10)
    
    def seleziona_cartella(self):
        """Seleziona la cartella con le foto."""
        print("[DEBUG] seleziona_cartella chiamata")
        cartella = filedialog.askdirectory(title="Seleziona cartella con le foto")
        print(f"[DEBUG] Cartella selezionata: {cartella}")
        if cartella:
            self.directory_selezionata = Path(cartella)
            print(f"[DEBUG] Path creato: {self.directory_selezionata}")
            print(f"[DEBUG] Path esiste: {self.directory_selezionata.exists()}")
            self.cartella_label.config(text=f"Cartella: {self.directory_selezionata.name}")
            self.leggi_foto()
        else:
            print("[DEBUG] Nessuna cartella selezionata dall'utente")
    
    def leggi_foto(self):
        """Legge tutte le foto dalla cartella (ottimizzato con multiprocessing)."""
        print(f"[DEBUG] leggi_foto chiamata, directory: {self.directory_selezionata}")
        if not self.directory_selezionata:
            print("[DEBUG] Nessuna directory selezionata!")
            return
        
        # Mostra progresso
        self.cartella_label.config(text="Lettura foto in corso...")
        self.root.update()
        
        self.foto_list = []
        
        # Trova tutti i file immagine
        estensioni = ['.jpg', '.JPG', '.jpeg', '.JPEG']
        foto_files = []
        for ext in estensioni:
            files = list(self.directory_selezionata.glob(f'*{ext}'))
            print(f"[DEBUG] Trovati {len(files)} file con estensione {ext}")
            foto_files.extend(files)
        
        foto_files.sort()
        print(f"[DEBUG] Totale file trovati: {len(foto_files)}")
        
        if not foto_files:
            self.cartella_label.config(text=f"Cartella: {self.directory_selezionata.name} (nessuna foto trovata)")
            return
        
        # Usa multiprocessing per leggere le foto in parallelo
        num_workers = min(multiprocessing.cpu_count(), len(foto_files))
        print(f"[DEBUG] Usando {num_workers} processi paralleli")
        
        foto_list_temp = []
        with ProcessPoolExecutor(max_workers=num_workers) as executor:
            # Invia tutti i task
            future_to_path = {
                executor.submit(leggi_foto_singola, foto_path): foto_path 
                for foto_path in foto_files
            }
            
            # Raccogli risultati man mano che completano
            completate = 0
            for future in as_completed(future_to_path):
                foto_path = future_to_path[future]
                try:
                    foto = future.result()
                    foto_list_temp.append(foto)
                    completate += 1
                    if completate % 10 == 0:
                        self.cartella_label.config(
                            text=f"Lettura foto: {completate}/{len(foto_files)}"
                        )
                        self.root.update()
                except Exception as e:
                    print(f"[DEBUG] Errore leggendo {foto_path.name}: {e}")
        
        self.foto_list = sorted(foto_list_temp, key=lambda f: f.nome_file)
        print(f"[DEBUG] Totale foto aggiunte alla lista: {len(self.foto_list)}")
        
        self.cartella_label.config(text=f"Cartella: {self.directory_selezionata.name}")
        self.aggiorna_tabella()
        self.aggiorna_statistiche()
    
    def aggiorna_tabella(self):
        """Aggiorna la tabella con i dati delle foto."""
        print(f"[DEBUG] aggiorna_tabella chiamata, foto_list ha {len(self.foto_list)} elementi")
        
        # Pulisci tabella
        items_prima = len(self.tree.get_children())
        print(f"[DEBUG] Righe nella tabella prima della pulizia: {items_prima}")
        for item in self.tree.get_children():
            self.tree.delete(item)
        
        # Aggiungi righe
        righe_aggiunte = 0
        for foto in self.foto_list:
            dt_original = foto.campi_exif['DateTimeOriginal']
            create_date = foto.campi_exif['CreateDate']
            modify_date = foto.campi_exif['ModifyDate']
            
            dt_original_str = dt_original.strftime('%Y-%m-%d %H:%M:%S') if dt_original else "❌"
            create_date_str = create_date.strftime('%Y-%m-%d %H:%M:%S') if create_date else "❌"
            modify_date_str = modify_date.strftime('%Y-%m-%d %H:%M:%S') if modify_date else "❌"
            
            strategia_str = STRATEGIE.get(foto.strategia, foto.strategia)
            
            item = self.tree.insert('', 'end', values=(
                foto.nome_file,
                dt_original_str,
                create_date_str,
                modify_date_str,
                strategia_str
            ))
            
            # Evidenzia se ci sono proposte
            if foto.proposte['DateTimeOriginal']:
                self.tree.set(item, 'DateTimeOriginal', 
                             f"→ {foto.proposte['DateTimeOriginal'].strftime('%Y-%m-%d %H:%M:%S')}")
                # Colora la riga
                self.tree.item(item, tags=('modificare',))
            
            righe_aggiunte += 1
        
        print(f"[DEBUG] Righe aggiunte alla tabella: {righe_aggiunte}")
        
        # Configura tag per evidenziare
        self.tree.tag_configure('modificare', background='#fff3cd')
        
        # Verifica finale
        items_dopo = len(self.tree.get_children())
        print(f"[DEBUG] Righe nella tabella dopo l'aggiornamento: {items_dopo}")
    
    def cambia_strategia_globale(self, event=None):
        """Cambia la strategia globale per tutte le foto."""
        strategia_selezionata = self.strategia_globale.get()
        
        # Trova la chiave corrispondente
        for key, value in STRATEGIE.items():
            if value == strategia_selezionata:
                for foto in self.foto_list:
                    foto.strategia = key
                break
        
        self.aggiorna_tabella()
    
    def on_double_click(self, event):
        """Gestisce il doppio click per cambiare strategia di un singolo file."""
        item = self.tree.selection()[0] if self.tree.selection() else None
        if not item:
            return
        
        # Trova la foto corrispondente
        nome_file = self.tree.item(item)['values'][0]
        foto = next((f for f in self.foto_list if f.nome_file == nome_file), None)
        if not foto:
            return
        
        # Crea finestra di dialogo per cambiare strategia
        dialog = tk.Toplevel(self.root)
        dialog.title("Cambia Strategia")
        dialog.geometry("400x200")
        
        ttk.Label(dialog, text=f"File: {nome_file}").pack(pady=10)
        ttk.Label(dialog, text="Seleziona strategia:").pack()
        
        strategia_var = tk.StringVar(value=STRATEGIE.get(foto.strategia, foto.strategia))
        combo = ttk.Combobox(dialog, textvariable=strategia_var, values=list(STRATEGIE.values()), 
                            state='readonly', width=30)
        combo.pack(pady=10)
        
        def applica():
            for key, value in STRATEGIE.items():
                if value == strategia_var.get():
                    foto.strategia = key
                    # Ricalcola proposte
                    foto.proposte = calcola_proposta(foto)
                    break
            dialog.destroy()
            self.aggiorna_tabella()
        
        ttk.Button(dialog, text="Applica", command=applica).pack(pady=10)
    
    def calcola_proposte(self):
        """Calcola le proposte di modifica per tutte le foto."""
        strategia_globale = self.strategia_globale.get()
        
        # Trova la chiave corrispondente
        strategia_key = None
        for key, value in STRATEGIE.items():
            if value == strategia_globale:
                strategia_key = key
                break
        
        if strategia_key:
            for foto in self.foto_list:
                foto.strategia = strategia_key
                foto.proposte = calcola_proposta(foto)
        
        self.aggiorna_tabella()
    
    def applica_modifiche(self):
        """Applica le modifiche a tutte le foto con proposte."""
        foto_da_modificare = [f for f in self.foto_list if f.proposte['DateTimeOriginal']]
        
        if not foto_da_modificare:
            messagebox.showinfo("Info", "Nessuna modifica da applicare!")
            return
        
        # Conferma
        risposta = messagebox.askyesno(
            "Conferma",
            f"Vuoi applicare le modifiche a {len(foto_da_modificare)} foto?"
        )
        
        if not risposta:
            return
        
        solo_dt = self.solo_datetime.get()
        successi = 0
        errori = 0
        
        for foto in foto_da_modificare:
            data = foto.proposte['DateTimeOriginal']
            if imposta_data_exif(foto.path, data, solo_datetime_original=solo_dt):
                successi += 1
                # Rileggi EXIF aggiornato
                foto.campi_exif = ottieni_tutti_campi_exif(foto.path)
                foto.proposte = {'DateTimeOriginal': None, 'CreateDate': None, 'ModifyDate': None}
            else:
                errori += 1
        
        messagebox.showinfo(
            "Completato",
            f"Modifiche applicate:\n✅ Successi: {successi}\n❌ Errori: {errori}"
        )
        
        self.aggiorna_tabella()
        self.aggiorna_statistiche()
    
    def aggiorna_statistiche(self):
        """Aggiorna le statistiche."""
        totale = len(self.foto_list)
        con_exif = sum(1 for f in self.foto_list if f.campi_exif['DateTimeOriginal'])
        con_proposte = sum(1 for f in self.foto_list if f.proposte['DateTimeOriginal'])
        
        stats = f"Totale foto: {totale}\nCon EXIF: {con_exif}\nCon proposte: {con_proposte}"
        self.stats_label.config(text=stats)

def main():
    root = tk.Tk()
    app = FotoCorrectorGUI(root)
    root.mainloop()

if __name__ == '__main__':
    main()

