use eframe::egui;
use std::path::PathBuf;
use std::fs;
use chrono::Datelike;
use crate::{FotoData, leggi_foto_da_directory};

#[derive(Clone, PartialEq)]
enum Strategia {
    NomeFile,
    JsonPhotoTaken, // photoTakenTime from JSON
    JsonCreation,   // creationTime from JSON
    ExifAttuale,
    NomeFilePreferito,
    JsonPreferito,  // Prefer photoTakenTime, otherwise creationTime
}

impl Strategia {
    fn as_str(&self) -> &str {
        match self {
            Strategia::NomeFile => "nome_file",
            Strategia::JsonPhotoTaken => "json_photo_taken",
            Strategia::JsonCreation => "json_creation",
            Strategia::ExifAttuale => "exif_attuale",
            Strategia::NomeFilePreferito => "nome_file_preferito",
            Strategia::JsonPreferito => "json_preferito",
        }
    }
    
    #[allow(dead_code)]
    fn from_str(s: &str) -> Self {
        match s {
            "nome_file" => Strategia::NomeFile,
            "json_photo_taken" => Strategia::JsonPhotoTaken,
            "json_creation" => Strategia::JsonCreation,
            "exif_attuale" => Strategia::ExifAttuale,
            "nome_file_preferito" => Strategia::NomeFilePreferito,
            "json_preferito" => Strategia::JsonPreferito,
            _ => Strategia::NomeFilePreferito,
        }
    }
    
    fn display_name(&self) -> &str {
        match self {
            Strategia::NomeFile => "Use year from filename",
            Strategia::JsonPhotoTaken => "Use photoTakenTime from JSON",
            Strategia::JsonCreation => "Use creationTime from JSON",
            Strategia::ExifAttuale => "Keep current EXIF",
            Strategia::NomeFilePreferito => "Prefer filename, otherwise JSON",
            Strategia::JsonPreferito => "Prefer JSON photoTakenTime, otherwise filename",
        }
    }
}

pub struct CorrectorApp {
    directory: Option<PathBuf>,
    ultima_cartella: Option<PathBuf>, // Ultima cartella aperta per riaprirla nel dialog
    foto_list: Vec<FotoData>,
    foto_selezionate: std::collections::HashSet<usize>, // Indici delle foto selezionate
    ultimo_indice_selezionato: Option<usize>, // Per gestire Shift+click
    strategia_datetime_original: Strategia,
    strategia_create_date: Strategia,
    loading: bool,
    loading_message: String,
    loading_progress: Option<(usize, usize)>, // (foto_trovate, foto_elaborate) per progresso
    loading_thread: Option<std::thread::JoinHandle<Vec<FotoData>>>, // Thread per caricamento asincrono
    loading_progress_receiver: Option<std::sync::mpsc::Receiver<usize>>, // Canale per ricevere progresso
    stats: String,
    foto_da_modificare_count: usize, // Number of photos to modify
    // State for progress bar
    applicando_modifiche: bool,
    foto_totali_da_modificare: usize,
    foto_modificate: usize,
    errori_applicazione: usize,
    // Shared counters for progress (used by write thread)
    progresso_counter: Option<std::sync::Arc<std::sync::Mutex<(usize, usize)>>>,
    // Filter for incongruity severity
    soglia_gravita_giorni: f32,
    unita_gravita: UnitaGravita,
    mostra_tutte_foto: bool, // Flag to show all photos, including those without incongruities
    solo_exif_mancante: bool, // Flag to show only photos with missing EXIF
    filtro_incongruenza: FiltroIncongruenza, // Filter by type of incongruity
    // Sorting
    colonna_ordinamento: Option<ColonnaOrdinamento>,
    ordine_crescente: bool,
    // Cached filtered list for performance
    foto_da_mostrare_cached: Vec<(usize, FotoData)>, // (indice_originale, foto)
    filtro_dirty: bool, // True when filters changed and cache needs refresh
    path_to_index: std::collections::HashMap<PathBuf, usize>, // Fast lookup map
}

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum ColonnaOrdinamento {
    NomeFile,
    Gravita,
    Incongruenze,
    DateTimeOriginal,
    CreateDate,
}

#[derive(Clone, Copy, PartialEq)]
enum UnitaGravita {
    Secondi,
    Minuti,
    Ore,
    Giorni,
    Mesi,
    Anni,
}

#[derive(Clone, Copy, PartialEq)]
enum FiltroIncongruenza {
    Tutte,
    SoloExifMancante,
    ExifAnnoDiversoFilename,
    ExifDiversoJson,
}

impl FiltroIncongruenza {
    fn display_name(&self) -> &str {
        match self {
            FiltroIncongruenza::Tutte => "Tutte le incongruenze",
            FiltroIncongruenza::SoloExifMancante => "Solo EXIF mancante",
            FiltroIncongruenza::ExifAnnoDiversoFilename => "EXIF anno ≠ filename",
            FiltroIncongruenza::ExifDiversoJson => "EXIF ≠ JSON photoTakenTime",
        }
    }
    
    fn matches(&self, foto: &crate::FotoData) -> bool {
        match self {
            FiltroIncongruenza::Tutte => !foto.incongruenze.is_empty(),
            FiltroIncongruenza::SoloExifMancante => {
                foto.incongruenze.iter().any(|inc| inc.contains("EXIF DateTimeOriginal mancante"))
            }
            FiltroIncongruenza::ExifAnnoDiversoFilename => {
                foto.incongruenze.iter().any(|inc| inc.contains("EXIF anno") && inc.contains("filename anno"))
            }
            FiltroIncongruenza::ExifDiversoJson => {
                foto.incongruenze.iter().any(|inc| inc.contains("EXIF") && inc.contains("JSON photoTakenTime"))
            }
        }
    }
}

impl UnitaGravita {
    fn display_name(&self) -> &str {
        match self {
            UnitaGravita::Secondi => "Seconds",
            UnitaGravita::Minuti => "Minutes",
            UnitaGravita::Ore => "Hours",
            UnitaGravita::Giorni => "Days",
            UnitaGravita::Mesi => "Months",
            UnitaGravita::Anni => "Years",
        }
    }
    
    fn to_giorni(&self, valore: f32) -> f32 {
        match self {
            UnitaGravita::Secondi => valore / 86400.0,
            UnitaGravita::Minuti => valore / 1440.0,
            UnitaGravita::Ore => valore / 24.0,
            UnitaGravita::Giorni => valore,
            UnitaGravita::Mesi => valore * 30.0,
            UnitaGravita::Anni => valore * 365.0,
        }
    }
}

impl CorrectorApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let ultima_cartella = Self::carica_ultima_cartella();
        Self {
            directory: None,
            ultima_cartella,
            foto_list: Vec::new(),
            foto_selezionate: std::collections::HashSet::new(),
            ultimo_indice_selezionato: None,
            strategia_datetime_original: Strategia::JsonPreferito, // JSON se disponibile, altrimenti filename
            strategia_create_date: Strategia::JsonPreferito, // JSON se disponibile, altrimenti filename
            loading: false,
            loading_message: String::new(),
            loading_progress: None,
            loading_thread: None,
            loading_progress_receiver: None,
            stats: String::new(),
            foto_da_modificare_count: 0,
            applicando_modifiche: false,
            foto_totali_da_modificare: 0,
            foto_modificate: 0,
            errori_applicazione: 0,
            progresso_counter: None,
            soglia_gravita_giorni: 0.0,
            unita_gravita: UnitaGravita::Giorni,
            mostra_tutte_foto: false,
            solo_exif_mancante: true, // Default: mostra solo EXIF mancante
            filtro_incongruenza: FiltroIncongruenza::Tutte,
            colonna_ordinamento: None,
            ordine_crescente: true,
            foto_da_mostrare_cached: Vec::new(),
            filtro_dirty: true,
            path_to_index: std::collections::HashMap::new(),
        }
    }
    
    fn percorso_config() -> Option<PathBuf> {
        std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join(".corrigi-exif-config.json"))
    }
    
    fn carica_ultima_cartella() -> Option<PathBuf> {
        let config_path = Self::percorso_config()?;
        let content = fs::read_to_string(&config_path).ok()?;
        let config: serde_json::Value = serde_json::from_str(&content).ok()?;
        let path_str = config.get("ultima_cartella")?.as_str()?;
        let path = PathBuf::from(path_str);
        // Verifica che la cartella esista ancora e prova a renderla canonica
        if path.exists() && path.is_dir() {
            // Prova a rendere il percorso canonico (assoluto)
            path.canonicalize().ok().or(Some(path))
        } else {
            None
        }
    }
    
    fn salva_ultima_cartella(path: &PathBuf) {
        if let Some(config_path) = Self::percorso_config() {
            // Assicurati che il percorso sia assoluto e canonico per il salvataggio
            let path_to_save = if let Ok(canonical) = path.canonicalize() {
                canonical
            } else {
                path.clone()
            };
            
            let config = serde_json::json!({
                "ultima_cartella": path_to_save.to_string_lossy().to_string()
            });
            if let Ok(json_str) = serde_json::to_string_pretty(&config) {
                if let Err(e) = fs::write(&config_path, json_str) {
                    eprintln!("Errore nel salvataggio della configurazione in {:?}: {}", config_path, e);
                } else {
                    eprintln!("Configurazione salvata in: {:?}", config_path);
                }
            }
        } else {
            eprintln!("Impossibile determinare il percorso HOME per salvare la configurazione");
        }
    }
    
    fn seleziona_cartella(&mut self) {
        // Su Linux, rfd potrebbe non supportare set_directory() correttamente con XDG Portal
        // Quindi cambiamo temporaneamente la directory di lavoro corrente
        let vecchia_dir = std::env::current_dir().ok();
        
        // Cambia alla directory dell'ultima cartella aperta, se disponibile
        if let Some(ref ultima_cartella) = self.ultima_cartella {
            if ultima_cartella.exists() && ultima_cartella.is_dir() {
                if let Ok(canonical_path) = ultima_cartella.canonicalize() {
                    if std::env::set_current_dir(&canonical_path).is_ok() {
                        eprintln!("Directory cambiata a: {:?}", canonical_path);
                    }
                } else if std::env::set_current_dir(ultima_cartella).is_ok() {
                    eprintln!("Directory cambiata a: {:?}", ultima_cartella);
                }
            }
        }
        
        // Apri il dialog (su Linux, molti dialog si aprono nella directory corrente)
        let mut dialog = rfd::FileDialog::new();
        
        // Prova anche con set_directory come fallback (potrebbe funzionare con GTK3 backend)
        if let Some(ref ultima_cartella) = self.ultima_cartella {
            if ultima_cartella.exists() && ultima_cartella.is_dir() {
                if let Ok(canonical_path) = ultima_cartella.canonicalize() {
                    if let Some(path_str) = canonical_path.to_str() {
                        dialog = dialog.set_directory(path_str);
                        eprintln!("Tentativo di impostare directory con set_directory: {}", path_str);
                    }
                } else if let Some(path_str) = ultima_cartella.to_str() {
                    dialog = dialog.set_directory(path_str);
                    eprintln!("Tentativo di impostare directory con set_directory: {}", path_str);
                }
            }
        }
        
        if let Some(path) = dialog.pick_folder() {
            eprintln!("Cartella selezionata: {:?}", path);
            self.directory = Some(path.clone());
            self.ultima_cartella = Some(path.clone());
            Self::salva_ultima_cartella(&path);
            self.avvia_caricamento_foto();
        }
        
        // Ripristina la directory di lavoro originale
        if let Some(ref vecchia) = vecchia_dir {
            let _ = std::env::set_current_dir(vecchia);
        }
    }
    
    fn avvia_caricamento_foto(&mut self) {
        if let Some(ref dir) = self.directory {
            self.loading = true;
            self.loading_message = format!("Scanning directory: {:?}...", dir);
            self.loading_progress = None;
            
            // Crea canale per comunicare progresso
            let (sender, receiver) = std::sync::mpsc::channel();
            self.loading_progress_receiver = Some(receiver);
            
            // Avvia il caricamento in un thread separato
            let dir_clone = dir.clone();
            let handle = std::thread::spawn(move || {
                eprintln!("[DEBUG] Inizio caricamento foto da: {:?}", dir_clone);
                crate::leggi_foto_da_directory_con_progresso(&dir_clone, Some(sender))
            });
            
            self.loading_thread = Some(handle);
        }
    }
    
    fn verifica_caricamento_completato(&mut self, ctx: &egui::Context) {
        if let Some(handle) = &self.loading_thread {
            if handle.is_finished() {
                // Prendi ownership del thread handle
                if let Some(handle) = self.loading_thread.take() {
                    match handle.join() {
                    Ok(foto_list) => {
                        eprintln!("[DEBUG] Caricamento completato: {} foto", foto_list.len());
                        self.foto_list = foto_list;
                        
                        // Ricostruisci la mappa path->indice per lookup veloce
                        eprintln!("[DEBUG] Costruzione mappa path->indice...");
                        self.path_to_index.clear();
                        for (idx, foto) in self.foto_list.iter().enumerate() {
                            self.path_to_index.insert(foto.path.clone(), idx);
                        }
                        eprintln!("[DEBUG] Mappa costruita con {} elementi", self.path_to_index.len());
                        
                        // Ricalcola le proposte con le strategie corrette della GUI
                        eprintln!("[DEBUG] Calcolo proposte...");
                        self.calcola_proposte();
                        eprintln!("[DEBUG] Proposte calcolate");
                        
                        eprintln!("[DEBUG] Aggiornamento statistiche...");
                        self.aggiorna_statistiche();
                        self.filtro_dirty = true; // Segna che il filtro deve essere ricalcolato
                        
                        self.loading = false;
                        self.loading_message.clear();
                        self.loading_progress = None;
                        
                        // Richiedi repaint per aggiornare la UI
                        ctx.request_repaint();
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Errore nel caricamento: {:?}", e);
                        self.loading = false;
                        self.loading_message = format!("Error loading photos: {:?}", e);
                    }
                }
                }
            } else {
                // Thread ancora in esecuzione - aggiorna il progresso
                if let Some(ref receiver) = self.loading_progress_receiver {
                    // Prova a ricevere aggiornamenti di progresso (non bloccante)
                    while let Ok(count) = receiver.try_recv() {
                        // Stima totale basata sul fatto che abbiamo già trovato le foto
                        // Per ora usiamo un placeholder, ma potremmo migliorare
                        if self.loading_progress.is_none() {
                            // Prima volta: stima che ci siano almeno count foto
                            self.loading_progress = Some((count, count));
                        } else {
                            // Aggiorna il conteggio elaborato
                            if let Some((trovate, _)) = self.loading_progress {
                                self.loading_progress = Some((trovate.max(count), count));
                            }
                        }
                    }
                }
                
                if let Some((trovate, elaborate)) = self.loading_progress {
                    self.loading_message = format!("Loading photos... Processing {}/{} files...", elaborate, trovate);
                } else {
                    self.loading_message = "Scanning directory and loading photos...".to_string();
                }
                // Richiedi repaint per aggiornare il messaggio
                ctx.request_repaint();
            }
        }
    }
    
    fn calcola_foto_da_mostrare(&mut self) {
        if !self.filtro_dirty && !self.foto_da_mostrare_cached.is_empty() {
            return; // Cache ancora valida
        }
        
        let soglia_secondi = (self.soglia_gravita_giorni * 86400.0) as i64;
        
        // Filtra le foto mantenendo l'indice originale
        let mut foto_filtrate: Vec<(usize, &FotoData)> = self.foto_list
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                // Filtro principale: solo EXIF mancante se abilitato
                if self.solo_exif_mancante {
                    // Mostra solo foto con DateTimeOriginal E CreateDate ENTRAMBI mancanti
                    if f.exif_datetime_original.is_some() || f.exif_create_date.is_some() {
                        return false; // Ha almeno uno dei campi EXIF, quindi escludi
                    }
                }
                
                // Filtro per tipo di incongruenza (solo se ci sono incongruenze)
                if !f.incongruenze.is_empty() {
                    if !self.filtro_incongruenza.matches(f) {
                        return false;
                    }
                } else {
                    // Se non ci sono incongruenze, mostra solo se:
                    // - mostra_tutte_foto è true, OPPURE
                    // - solo_exif_mancante è true (per mostrare quelle con EXIF mancante anche senza incongruenze)
                    if !self.mostra_tutte_foto && !self.solo_exif_mancante {
                        return false;
                    }
                    // Se solo_exif_mancante è true ma non ci sono incongruenze,
                    // mostra solo se ha EXIF mancante (già controllato sopra)
                    if self.solo_exif_mancante {
                        return true; // Ha EXIF mancante (già verificato sopra)
                    }
                }
                
                // Se mostra_tutte_foto è true, mostra anche quelle senza incongruenze
                if self.mostra_tutte_foto {
                    if f.incongruenze.is_empty() {
                        return true; // Mostra anche quelle senza incongruenze
                    }
                    // Per quelle con incongruenze, applica il filtro sulla soglia
                    let gravita_secondi = (f.gravita_incongruenza as f64 * 86400.0) as i64;
                    gravita_secondi >= soglia_secondi
                } else {
                    // Mostra solo quelle con incongruenze che superano la soglia
                    if f.incongruenze.is_empty() {
                        // Se solo_exif_mancante è true, mostra anche quelle con EXIF mancante senza incongruenze
                        if self.solo_exif_mancante {
                            return true;
                        }
                        return false;
                    }
                    // Converti la gravità della foto in secondi per il confronto
                    let gravita_secondi = (f.gravita_incongruenza as f64 * 86400.0) as i64;
                    gravita_secondi >= soglia_secondi
                }
            })
            .collect();
        
        // Applica ordinamento se selezionato
        if let Some(colonna) = self.colonna_ordinamento {
            foto_filtrate.sort_by(|(_, a), (_, b)| {
                let cmp = match colonna {
                    ColonnaOrdinamento::NomeFile => {
                        a.nome_file.cmp(&b.nome_file)
                    }
                    ColonnaOrdinamento::Gravita => {
                        a.gravita_incongruenza.cmp(&b.gravita_incongruenza)
                    }
                    ColonnaOrdinamento::Incongruenze => {
                        a.incongruenze.len().cmp(&b.incongruenze.len())
                    }
                    ColonnaOrdinamento::DateTimeOriginal => {
                        match (a.exif_datetime_original, b.exif_datetime_original) {
                            (Some(dt_a), Some(dt_b)) => dt_a.cmp(&dt_b),
                            (Some(_), None) => std::cmp::Ordering::Less,
                            (None, Some(_)) => std::cmp::Ordering::Greater,
                            (None, None) => std::cmp::Ordering::Equal,
                        }
                    }
                    ColonnaOrdinamento::CreateDate => {
                        match (a.exif_create_date, b.exif_create_date) {
                            (Some(dt_a), Some(dt_b)) => dt_a.cmp(&dt_b),
                            (Some(_), None) => std::cmp::Ordering::Less,
                            (None, Some(_)) => std::cmp::Ordering::Greater,
                            (None, None) => std::cmp::Ordering::Equal,
                        }
                    }
                };
                
                if self.ordine_crescente {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
        }
        
        // Converti in owned data per la cache
        self.foto_da_mostrare_cached = foto_filtrate.into_iter()
            .map(|(idx, foto)| (idx, foto.clone()))
            .collect();
        
        self.filtro_dirty = false;
    }
    
    fn calcola_proposte(&mut self) {
        for foto in &mut self.foto_list {
            foto.strategia_datetime_original = self.strategia_datetime_original.as_str().to_string();
            foto.strategia_create_date = self.strategia_create_date.as_str().to_string();
            
            foto.proposta_datetime_original = crate::calcola_proposta_con_strategia(foto, &foto.strategia_datetime_original);
            foto.proposta_create_date = crate::calcola_proposta_con_strategia(foto, &foto.strategia_create_date);
            
            // Recalculate incongruities and severity after calculating proposals
            foto.incongruenze = crate::rileva_incongruenze(foto);
            foto.gravita_incongruenza = crate::calcola_gravita_incongruenza(foto);
        }
        self.filtro_dirty = true; // Le proposte cambiate possono influenzare il filtro
    }
    
    fn avvia_applicazione_modifiche(&mut self, _ctx: &egui::Context) {
        // Apply modifications only to selected photos
        let foto_da_modificare: Vec<_> = self.foto_list
            .iter()
            .enumerate()
            .filter(|(idx, f)| {
                // Must be selected AND have at least one proposal
                self.foto_selezionate.contains(idx) && (
                    f.proposta_datetime_original.is_some() ||
                    f.proposta_create_date.is_some()
                )
            })
            .map(|(_, f)| f.clone())
            .collect();
        
        if foto_da_modificare.is_empty() {
            return;
        }
        
        self.foto_totali_da_modificare = foto_da_modificare.len();
        self.foto_modificate = 0;
        self.errori_applicazione = 0;
        self.applicando_modifiche = true;
        
        // Prepara i dati per la scrittura parallela
        let dati_scrittura: Vec<_> = foto_da_modificare.iter().map(|foto| {
            let mut campi_da_scrivere = Vec::new();
            
            if let Some(data) = foto.proposta_datetime_original {
                campi_da_scrivere.push(("DateTimeOriginal", data));
            }
            if let Some(data) = foto.proposta_create_date {
                campi_da_scrivere.push(("CreateDate", data));
            }
            
            (foto.path.clone(), campi_da_scrivere)
        }).collect();
        
        // Use shared counters to communicate progress
        use std::sync::{Arc, Mutex};
        let progresso = Arc::new(Mutex::new((0usize, 0usize))); // (successi, errori)
        self.progresso_counter = Some(progresso.clone());
        
        // Start writing in a separate thread
        std::thread::spawn(move || {
            use rayon::prelude::*;
            
            let _risultati: Vec<_> = dati_scrittura
                .into_par_iter()
                .map(|(path, campi)| {
                    let risultato = if campi.is_empty() {
                        Ok(())
                    } else {
                        crate::scrivi_tutti_campi_exif(&path, &campi)
                    };
                    
                    // Update shared counters
                    let mut counter = progresso.lock().unwrap();
                    if risultato.is_ok() {
                        counter.0 += 1;
                    } else {
                        counter.1 += 1;
                        // Print error for debug
                        if let Err(ref e) = risultato {
                            eprintln!("EXIF write error for {}: {}", path.display(), e);
                        }
                    }
                    
                    risultato
                })
                .collect();
            
            // Mark as completed by setting a special value
            // (we'll use foto_totali_da_modificare + 1 as completion indicator)
        });
    }
    
    fn aggiorna_progresso_da_counter(&mut self, ctx: &egui::Context) {
        let mut completato = false;
        
        if let Some(ref counter_arc) = self.progresso_counter {
            if let Ok(counter) = counter_arc.try_lock() {
                let (successi, errori) = *counter;
                let totale_elaborate = successi + errori;
                
                // Update state
                self.foto_modificate = successi;
                self.errori_applicazione = errori;
                
                // If all photos have been processed, complete
                if totale_elaborate >= self.foto_totali_da_modificare && self.foto_totali_da_modificare > 0 {
                    completato = true;
                } else {
                    ctx.request_repaint();
                }
            }
        }
        
        // Release borrow before modifying self
        if completato {
            self.applicando_modifiche = false;
            self.progresso_counter = None;
            
            // Reload photos after modifications
            if let Some(ref dir) = self.directory {
                // Save paths of selected photos before reloading
                let vecchie_selezioni: Vec<_> = self.foto_selezionate.iter()
                    .filter_map(|idx| self.foto_list.get(*idx).map(|f| f.path.clone()))
                    .collect();
                
                self.foto_list = leggi_foto_da_directory(dir);
                
                // Ricostruisci la mappa path->indice
                self.path_to_index.clear();
                for (idx, foto) in self.foto_list.iter().enumerate() {
                    self.path_to_index.insert(foto.path.clone(), idx);
                }
                
                // Restore selections based on path
                self.foto_selezionate.clear();
                for path in vecchie_selezioni {
                    if let Some(&idx) = self.path_to_index.get(&path) {
                        self.foto_selezionate.insert(idx);
                    }
                }
                
                // IMPORTANTE: Ricalcola le proposte con le strategie corrette della GUI
                // dopo il reload, altrimenti rimangono quelle di default
                self.calcola_proposte();
                self.aggiorna_statistiche();
                self.filtro_dirty = true; // Ricalcola il filtro dopo il reload
            }
        }
    }
    
    fn aggiorna_statistiche(&mut self) {
        let totale = self.foto_list.len();
        let con_exif = self.foto_list.iter()
            .filter(|f| f.exif_datetime_original.is_some())
            .count();
        let con_proposte = self.foto_list.iter()
            .filter(|f| f.proposta_datetime_original.is_some())
            .count();
        
        self.stats = format!(
            "Total photos: {}\nWith EXIF: {}\nWith proposals: {}",
            totale, con_exif, con_proposte
        );
    }
}

impl eframe::App for CorrectorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Verifica se il caricamento è completato
        self.verifica_caricamento_completato(ctx);
        
        // Confirmation dialog rimosso - applica direttamente le modifiche
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("EXIF Date Corrector - Rust Version");
            
            ui.separator();
            
            // Phase 1: Folder selection
            ui.horizontal(|ui| {
                ui.label("Phase 1: Read Folder");
                if ui.button("Select Folder").clicked() {
                    self.seleziona_cartella();
                }
                if let Some(ref dir) = self.directory {
                    ui.label(format!("Folder: {}", dir.file_name().unwrap_or_default().to_string_lossy()));
                } else {
                    ui.label("No folder selected");
                }
            });
            
            // Mostra indicatore di caricamento se sta caricando - MOLTO VISIBILE
            if self.loading {
                ui.separator();
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width(), 100.0),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(10.0);
                            ui.spinner();
                            ui.add_space(10.0);
                            ui.heading(&self.loading_message);
                            ui.add_space(10.0);
                            
                            if let Some((trovate, elaborate)) = self.loading_progress {
                                if elaborate <= trovate && trovate > 0 {
                                    let progresso = elaborate as f32 / trovate as f32;
                                    ui.add(egui::ProgressBar::new(progresso)
                                        .show_percentage()
                                        .desired_width(ui.available_width() - 40.0));
                                    ui.label(format!("Processing: {}/{} photos ({:.1}%)", 
                                                   elaborate, trovate, progresso * 100.0));
                                }
                            } else {
                                // Progress bar indeterminata durante lo scan
                                ui.add(egui::ProgressBar::new(0.0)
                                    .show_percentage()
                                    .desired_width(ui.available_width() - 40.0));
                                ui.label("Scanning directory...");
                            }
                        });
                    }
                );
                ui.separator();
            }
            
            ui.separator();
            
            // Filter by severity with slider that adapts to selected unit
            ui.horizontal(|ui| {
                ui.label("Filter by minimum severity:");
                
                // Calculate current value in selected unit
                // Threshold is stored in seconds to allow precision up to seconds
                let mut valore_unita = match self.unita_gravita {
                    UnitaGravita::Secondi => self.soglia_gravita_giorni * 86400.0,
                    UnitaGravita::Minuti => self.soglia_gravita_giorni * 1440.0,
                    UnitaGravita::Ore => self.soglia_gravita_giorni * 24.0,
                    UnitaGravita::Giorni => self.soglia_gravita_giorni,
                    UnitaGravita::Mesi => self.soglia_gravita_giorni / 30.0,
                    UnitaGravita::Anni => self.soglia_gravita_giorni / 365.0,
                };
                
                // Calculate slider range based on unit
                // Minimum is always 0 (which corresponds to 0 seconds)
                let (min_val, max_val) = match self.unita_gravita {
                    UnitaGravita::Secondi => (0.0, 31536000.0), // 0 - 1 year in seconds
                    UnitaGravita::Minuti => (0.0, 525600.0), // 0 - 1 year in minutes
                    UnitaGravita::Ore => (0.0, 8760.0), // 0 - 1 year in hours
                    UnitaGravita::Giorni => (0.0, 3650.0), // 0 - 10 years (but can be converted to seconds)
                    UnitaGravita::Mesi => (0.0, 120.0), // 0 - 10 years in months
                    UnitaGravita::Anni => (0.0, 10.0), // 0 - 10 years
                };
                
                // Ensure minimum value is at least 0 (0 seconds)
                valore_unita = valore_unita.max(0.0);
                
                let unita_text = self.unita_gravita.display_name().to_lowercase();
                // Create slider text using current value (before mutable borrow)
                let slider_text = format!("{:.2} {}", valore_unita, unita_text);
                ui.add(egui::Slider::new(&mut valore_unita, min_val..=max_val)
                    .text(slider_text));
                
                // Convert selected unit value to days (but maintains precision up to seconds)
                // Value is converted to days, but can represent fractions of seconds
                let vecchio_valore = self.soglia_gravita_giorni;
                let nuovo_valore_giorni = self.unita_gravita.to_giorni(valore_unita);
                // Ensure it's not negative (minimum 0 seconds = 0 days)
                self.soglia_gravita_giorni = nuovo_valore_giorni.max(0.0);
                if (vecchio_valore - self.soglia_gravita_giorni).abs() > 0.001 {
                    self.filtro_dirty = true; // Soglia cambiata
                }
            });
            
            ui.horizontal(|ui| {
                ui.label("Unit:");
                let vecchia_unita = self.unita_gravita;
                egui::ComboBox::from_id_source("unita_gravita")
                    .selected_text(self.unita_gravita.display_name())
                    .show_ui(ui, |ui| {
                        for unita in [
                            UnitaGravita::Secondi,
                            UnitaGravita::Minuti,
                            UnitaGravita::Ore,
                            UnitaGravita::Giorni,
                            UnitaGravita::Mesi,
                            UnitaGravita::Anni,
                        ] {
                            ui.selectable_value(&mut self.unita_gravita, unita, unita.display_name());
                        }
                    });
                if vecchia_unita != self.unita_gravita {
                    self.filtro_dirty = true; // Unità cambiata (anche se la soglia è la stessa, il display cambia)
                }
            });
            
            ui.separator();
            
            // Checkbox to show only photos with missing EXIF (checked by default)
            let vecchio_solo_exif = self.solo_exif_mancante;
            ui.checkbox(&mut self.solo_exif_mancante, "Mostra solo foto con EXIF mancante");
            if vecchio_solo_exif != self.solo_exif_mancante {
                self.filtro_dirty = true;
            }
            
            // Menu a tendina per filtrare per tipo di incongruenza
            ui.horizontal(|ui| {
                ui.label("Filtra per tipo di incongruenza:");
                let vecchio_filtro = self.filtro_incongruenza;
                egui::ComboBox::from_id_source("filtro_incongruenza")
                    .selected_text(self.filtro_incongruenza.display_name())
                    .show_ui(ui, |ui| {
                        for filtro in [
                            FiltroIncongruenza::Tutte,
                            FiltroIncongruenza::SoloExifMancante,
                            FiltroIncongruenza::ExifAnnoDiversoFilename,
                            FiltroIncongruenza::ExifDiversoJson,
                        ] {
                            ui.selectable_value(&mut self.filtro_incongruenza, filtro, filtro.display_name());
                        }
                    });
                if vecchio_filtro != self.filtro_incongruenza {
                    self.filtro_dirty = true;
                }
            });
            
            ui.separator();
            
            // Checkbox to show all photos (deprecated, mantenuto per compatibilità)
            let vecchia_mostra_tutte = self.mostra_tutte_foto;
            ui.checkbox(&mut self.mostra_tutte_foto, "Show all photos (including those without incongruities)");
            if vecchia_mostra_tutte != self.mostra_tutte_foto {
                self.filtro_dirty = true;
            }
            
            ui.separator();
            
            // Calcola la lista filtrata (usa cache se disponibile)
            self.calcola_foto_da_mostrare();
            
            // Show threshold in selected unit
            let soglia_secondi = (self.soglia_gravita_giorni * 86400.0) as i64;
            let soglia_display = match self.unita_gravita {
                UnitaGravita::Secondi => format!("{} seconds", soglia_secondi),
                UnitaGravita::Minuti => format!("{:.2} minutes", self.soglia_gravita_giorni * 1440.0),
                UnitaGravita::Ore => format!("{:.2} hours", self.soglia_gravita_giorni * 24.0),
                UnitaGravita::Giorni => format!("{:.2} days", self.soglia_gravita_giorni),
                UnitaGravita::Mesi => format!("{:.2} months", self.soglia_gravita_giorni / 30.0),
                UnitaGravita::Anni => format!("{:.2} years", self.soglia_gravita_giorni / 365.0),
            };
            
            if self.mostra_tutte_foto {
                ui.label(format!("Photos shown: {} (all, filter >= {} for incongruities)", self.foto_da_mostrare_cached.len(), soglia_display));
            } else {
                ui.label(format!("Photos with incongruities >= {}: {}", soglia_display, self.foto_da_mostrare_cached.len()));
            }
            
            ui.label(format!("Selected photos: {}", self.foto_selezionate.len()));
            ui.separator();
            
            egui::ScrollArea::both().show(ui, |ui| {
                egui::Grid::new("foto_grid")
                    .num_columns(8)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        // Header with checkbox "Select all"
                        let tutte_selezionate = !self.foto_da_mostrare_cached.is_empty() && 
                            self.foto_da_mostrare_cached.iter().all(|(idx_originale, _)| {
                                self.foto_selezionate.contains(idx_originale)
                            });
                        let mut seleziona_tutte = tutte_selezionate;
                        if ui.checkbox(&mut seleziona_tutte, "Select").changed() {
                            // Select/deselect all visible photos
                            for (idx_originale, _) in self.foto_da_mostrare_cached.iter() {
                                if seleziona_tutte {
                                    self.foto_selezionate.insert(*idx_originale);
                                } else {
                                    self.foto_selezionate.remove(idx_originale);
                                }
                            }
                        }
                        // Clickable headers for sorting
                        let nome_response = ui.selectable_label(
                            self.colonna_ordinamento == Some(ColonnaOrdinamento::NomeFile),
                            "File Name"
                        );
                        if nome_response.clicked() {
                            if self.colonna_ordinamento == Some(ColonnaOrdinamento::NomeFile) {
                                self.ordine_crescente = !self.ordine_crescente;
                                self.filtro_dirty = true; // Ordinamento cambiato
                            } else {
                                self.colonna_ordinamento = Some(ColonnaOrdinamento::NomeFile);
                                self.ordine_crescente = true;
                                self.filtro_dirty = true; // Ordinamento cambiato
                            }
                        }
                        
                        let gravita_response = ui.selectable_label(
                            self.colonna_ordinamento == Some(ColonnaOrdinamento::Gravita),
                            "Severity"
                        );
                        if gravita_response.clicked() {
                            if self.colonna_ordinamento == Some(ColonnaOrdinamento::Gravita) {
                                self.ordine_crescente = !self.ordine_crescente;
                                self.filtro_dirty = true; // Ordinamento cambiato
                            } else {
                                self.colonna_ordinamento = Some(ColonnaOrdinamento::Gravita);
                                self.ordine_crescente = true;
                                self.filtro_dirty = true; // Ordinamento cambiato
                            }
                        }
                        
                        let mut inc_label_text = "Incongruities".to_string();
                        if self.colonna_ordinamento == Some(ColonnaOrdinamento::Incongruenze) {
                            let arrow = if self.ordine_crescente { " ↑" } else { " ↓" };
                            inc_label_text.push_str(arrow);
                        }
                        let inc_header_response = ui.button(inc_label_text);
                        if inc_header_response.clicked() {
                            if self.colonna_ordinamento == Some(ColonnaOrdinamento::Incongruenze) {
                                self.ordine_crescente = !self.ordine_crescente;
                                self.filtro_dirty = true; // Ordinamento cambiato
                            } else {
                                self.colonna_ordinamento = Some(ColonnaOrdinamento::Incongruenze);
                                self.ordine_crescente = true;
                                self.filtro_dirty = true; // Ordinamento cambiato
                            }
                        }
                        
                        let dt_response = ui.selectable_label(
                            self.colonna_ordinamento == Some(ColonnaOrdinamento::DateTimeOriginal),
                            "DateTimeOriginal ⭐"
                        );
                        if dt_response.clicked() {
                            if self.colonna_ordinamento == Some(ColonnaOrdinamento::DateTimeOriginal) {
                                self.ordine_crescente = !self.ordine_crescente;
                                self.filtro_dirty = true; // Ordinamento cambiato
                            } else {
                                self.colonna_ordinamento = Some(ColonnaOrdinamento::DateTimeOriginal);
                                self.ordine_crescente = true;
                                self.filtro_dirty = true; // Ordinamento cambiato
                            }
                        }
                        
                        ui.label("→ Proposal");
                        
                        let cd_response = ui.selectable_label(
                            self.colonna_ordinamento == Some(ColonnaOrdinamento::CreateDate),
                            "CreateDate"
                        );
                        if cd_response.clicked() {
                            if self.colonna_ordinamento == Some(ColonnaOrdinamento::CreateDate) {
                                self.ordine_crescente = !self.ordine_crescente;
                                self.filtro_dirty = true; // Ordinamento cambiato
                            } else {
                                self.colonna_ordinamento = Some(ColonnaOrdinamento::CreateDate);
                                self.ordine_crescente = true;
                                self.filtro_dirty = true; // Ordinamento cambiato
                            }
                        }
                        
                        ui.label("→ Proposal");
                        ui.end_row();
                        
                        // Data rows - filtered photos (usa cache con indici già calcolati)
                        for (idx_grid, (idx_originale, foto)) in self.foto_da_mostrare_cached.iter().enumerate() {
                            
                            let mut is_selected = self.foto_selezionate.contains(&idx_originale);
                            
                            // Checkbox for selection with Ctrl/Shift handling
                            let checkbox_response = ui.checkbox(&mut is_selected, "");
                            
                            if checkbox_response.changed() {
                                let input = ui.input(|i| i.clone());
                                let ctrl_pressed = input.modifiers.ctrl;
                                let shift_pressed = input.modifiers.shift;
                                
                                if shift_pressed && self.ultimo_indice_selezionato.is_some() {
                                    // Shift+click: select range
                                    let ultimo_idx = self.ultimo_indice_selezionato.unwrap();
                                    let start_idx = self.foto_da_mostrare_cached.iter()
                                        .position(|(idx, _)| *idx == ultimo_idx)
                                        .unwrap_or(0);
                                    let end_idx = idx_grid;
                                    let (start, end) = if start_idx < end_idx {
                                        (start_idx, end_idx)
                                    } else {
                                        (end_idx, start_idx)
                                    };
                                    
                                    for i in start..=end {
                                        if let Some((idx, _)) = self.foto_da_mostrare_cached.get(i) {
                                            self.foto_selezionate.insert(*idx);
                                        }
                                    }
                                    self.ultimo_indice_selezionato = Some(*idx_originale);
                                } else if ctrl_pressed {
                                    // Ctrl+click: add/remove single photo
                                    if is_selected {
                                        self.foto_selezionate.insert(*idx_originale);
                                    } else {
                                        self.foto_selezionate.remove(idx_originale);
                                    }
                                    self.ultimo_indice_selezionato = Some(*idx_originale);
                                } else {
                                    // Normal click: select only this photo
                                    self.foto_selezionate.clear();
                                    self.foto_selezionate.insert(*idx_originale);
                                    self.ultimo_indice_selezionato = Some(*idx_originale);
                                }
                            } else if is_selected {
                                // Keep selected state
                                self.foto_selezionate.insert(*idx_originale);
                            } else {
                                self.foto_selezionate.remove(idx_originale);
                            }
                            
                            // Highlight row if selected
                            if is_selected {
                                ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(100, 150, 255));
                            }
                            
                            // File name - make it clickable for double click
                            let nome_response = ui.selectable_label(false, &foto.nome_file);
                            
                            // Handle double click on file name to open photo
                            if nome_response.double_clicked() {
                                let foto_path = foto.path.clone();
                                std::thread::spawn(move || {
                                    let _ = std::process::Command::new("xdg-open")
                                        .arg(&foto_path)
                                        .spawn();
                                });
                            }
                            
                            // Severity with thermometric scale
                            let giorni_diff = foto.gravita_incongruenza;
                            let gravita_text = if foto.incongruenze.is_empty() {
                                "None".to_string()
                            } else if giorni_diff == 0 {
                                "OK".to_string()
                            } else if giorni_diff < 30 {
                                format!("{} days", giorni_diff)
                            } else if giorni_diff < 365 {
                                format!("{} months", giorni_diff / 30)
                            } else {
                                format!("{} years", giorni_diff / 365)
                            };
                            
                            // Calculate thermometric color (green -> yellow -> red)
                            let colore = if foto.incongruenze.is_empty() {
                                egui::Color32::from_rgb(150, 150, 150) // Gray for no incongruity
                            } else if giorni_diff == 0 {
                                egui::Color32::from_rgb(0, 200, 0) // Green
                            } else if giorni_diff < 30 {
                                egui::Color32::from_rgb(100, 200, 0) // Green-yellow
                            } else if giorni_diff < 90 {
                                egui::Color32::from_rgb(200, 200, 0) // Yellow
                            } else if giorni_diff < 365 {
                                egui::Color32::from_rgb(255, 150, 0) // Orange
                            } else {
                                egui::Color32::from_rgb(255, 0, 0) // Red
                            };
                            
                            ui.visuals_mut().override_text_color = Some(colore);
                            ui.label(gravita_text);
                            ui.visuals_mut().override_text_color = None;
                            
                            // Incongruities - show "None" if empty, otherwise make it clickable
                            let inc_text = if foto.incongruenze.is_empty() {
                                "None".to_string()
                            } else {
                                foto.incongruenze.join("; ")
                            };
                            
                            // If there are incongruities, make the text clickable to open JSON
                            if !foto.incongruenze.is_empty() {
                                let inc_response = ui.selectable_label(false, &inc_text);
                                if inc_response.clicked() {
                                    // Find corresponding JSON file
                                    let json_path = crate::trova_file_json(&foto.path);
                                    if let Some(json_path) = json_path {
                                        let json_path_clone = json_path.clone();
                                        std::thread::spawn(move || {
                                            let _ = std::process::Command::new("xdg-open")
                                                .arg(&json_path_clone)
                                                .spawn();
                                        });
                                    }
                                }
                            } else {
                                ui.label(inc_text);
                            }
                            
                            // Current DateTimeOriginal
                            if let Some(dt) = foto.exif_datetime_original {
                                ui.label(dt.format("%Y-%m-%d %H:%M:%S").to_string());
                            } else {
                                ui.label("❌");
                            }
                            
                            // DateTimeOriginal proposal
                            if let Some(dt_proposta) = foto.proposta_datetime_original {
                                let testo_proposta = format!("→ {}", dt_proposta.format("%Y-%m-%d %H:%M:%S"));
                                // Check if this is the "flag" date 1900-01-01 for photos without metadata
                                let is_flag_date = dt_proposta.year() == 1900 && dt_proposta.month() == 1 && dt_proposta.day() == 1;
                                
                                if is_flag_date {
                                    // Red/purple color to indicate this is a flag date for manual classification
                                    ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(255, 0, 255)); // Magenta
                                } else {
                                    // Compare with current EXIF to decide color
                                    let cambia = match foto.exif_datetime_original {
                                        Some(dt_exif) => dt_exif != dt_proposta,
                                        None => true, // If EXIF missing, consider it as a change
                                    };
                                    if cambia {
                                        // Orange color to indicate modification
                                        ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(255, 165, 0)); // Orange
                                    } else {
                                        // Gray if no change
                                        ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(150, 150, 150)); // Gray
                                    }
                                }
                                ui.label(testo_proposta);
                                ui.visuals_mut().override_text_color = None; // Reset
                            } else {
                                ui.label("-");
                            }
                            
                            // Current CreateDate
                            if let Some(dt) = foto.exif_create_date {
                                ui.label(dt.format("%Y-%m-%d %H:%M:%S").to_string());
                            } else {
                                ui.label("❌");
                            }
                            
                            // CreateDate proposal
                            if let Some(dt_proposta) = foto.proposta_create_date {
                                let testo_proposta = format!("→ {}", dt_proposta.format("%Y-%m-%d %H:%M:%S"));
                                // Check if this is the "flag" date 1900-01-01 for photos without metadata
                                let is_flag_date = dt_proposta.year() == 1900 && dt_proposta.month() == 1 && dt_proposta.day() == 1;
                                
                                if is_flag_date {
                                    // Red/purple color to indicate this is a flag date for manual classification
                                    ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(255, 0, 255)); // Magenta
                                } else {
                                    // Compare with current EXIF to decide color
                                    let cambia = match foto.exif_create_date {
                                        Some(dt_exif) => dt_exif != dt_proposta,
                                        None => true, // If EXIF missing, consider it as a change
                                    };
                                    if cambia {
                                        // Orange color to indicate modification
                                        ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(255, 165, 0)); // Orange
                                    } else {
                                        // Gray if no change
                                        ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(150, 150, 150)); // Gray
                                    }
                                }
                                ui.label(testo_proposta);
                                ui.visuals_mut().override_text_color = None; // Reset
                            } else {
                                ui.label("-");
                            }
                            
                            // Reset color at end of row
                            ui.visuals_mut().override_text_color = None;
                            ui.end_row();
                        }
                    });
            });
        });
        
        // Right side panel
        egui::SidePanel::right("controlli").show(ctx, |ui| {
            ui.heading("Controls");
            
            ui.separator();
            
            // Phase 2: Proposal modifications
            ui.group(|ui| {
                ui.label("Phase 2: Proposal Modifications");
                ui.separator();
                
                let mut strategia_cambiata = false;
                
                ui.label("DateTimeOriginal ⭐ Strategy:");
                egui::ComboBox::from_id_source("strategia_dt")
                    .selected_text(self.strategia_datetime_original.display_name())
                    .show_ui(ui, |ui| {
                        for strategia in [
                            Strategia::JsonPhotoTaken,
                            Strategia::JsonCreation,
                            Strategia::NomeFilePreferito,
                            Strategia::NomeFile,
                            Strategia::JsonPreferito,
                            Strategia::ExifAttuale,
                        ] {
                            if ui.selectable_value(&mut self.strategia_datetime_original, strategia.clone(), strategia.display_name()).changed() {
                                strategia_cambiata = true;
                            }
                        }
                    });
                
                ui.separator();
                
                ui.label("CreateDate Strategy:");
                egui::ComboBox::from_id_source("strategia_cd")
                    .selected_text(self.strategia_create_date.display_name())
                    .show_ui(ui, |ui| {
                        for strategia in [
                            Strategia::JsonPhotoTaken,
                            Strategia::JsonCreation,
                            Strategia::NomeFilePreferito,
                            Strategia::NomeFile,
                            Strategia::JsonPreferito,
                            Strategia::ExifAttuale,
                        ] {
                            if ui.selectable_value(&mut self.strategia_create_date, strategia.clone(), strategia.display_name()).changed() {
                                strategia_cambiata = true;
                            }
                        }
                    });
                
                ui.separator();
                
                // If any strategy changed, automatically recalculate proposals
                if strategia_cambiata {
                    self.calcola_proposte();
                }
                
                ui.label("Proposals are automatically recalculated when you change strategies.");
            });
            
            ui.separator();
            
            // Phase 3: Apply modifications
            ui.group(|ui| {
                ui.label("Phase 3: Apply Modifications");
                ui.separator();
                
                ui.label("Modifications will be applied only to selected photos.");
                ui.label("Each EXIF field can have an independent strategy.");
                
                let foto_selezionate_count = self.foto_selezionate.len();
                if foto_selezionate_count == 0 {
                    ui.label("⚠️ No photos selected!");
                } else {
                    ui.label(format!("✅ {} photos selected", foto_selezionate_count));
                }
                
                if ui.button("Apply Modifications").clicked() {
                    if foto_selezionate_count > 0 {
                        // Calculate how many selected photos actually have proposals to apply
                        let foto_con_proposte = self.foto_list
                            .iter()
                            .enumerate()
                            .filter(|(idx, f)| {
                                self.foto_selezionate.contains(idx) && (
                                    f.proposta_datetime_original.is_some() ||
                                    f.proposta_create_date.is_some()
                                )
                            })
                            .count();
                        
                        if foto_con_proposte > 0 {
                            self.foto_da_modificare_count = foto_con_proposte;
                            // Applica direttamente le modifiche senza dialog di conferma
                            self.avvia_applicazione_modifiche(ctx);
                        }
                    }
                }
                
                // Show progress bar if applying modifications
                if self.applicando_modifiche {
                    ui.separator();
                    ui.label("Applying modifications...");
                    let progresso = if self.foto_totali_da_modificare > 0 {
                        self.foto_modificate as f32 / self.foto_totali_da_modificare as f32
                    } else {
                        0.0
                    };
                    ui.add(egui::ProgressBar::new(progresso).show_percentage());
                    ui.label(format!("{}/{} photos processed", self.foto_modificate, self.foto_totali_da_modificare));
                    
                    // Update progress from shared counter
                    self.aggiorna_progresso_da_counter(ctx);
                } else if self.foto_modificate > 0 {
                    ui.separator();
                    ui.label(format!("✅ Completed: {} photos modified", self.foto_modificate));
                    if self.errori_applicazione > 0 {
                        ui.label(format!("⚠️ Errors: {}", self.errori_applicazione));
                    }
                }
            });
            
            ui.separator();
            
            // Statistiche
            ui.label("Statistiche:");
            ui.label(&self.stats);
        });
    }
}

