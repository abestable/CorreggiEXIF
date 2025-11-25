use eframe::egui;
use std::path::PathBuf;
use crate::{FotoData, leggi_foto_da_directory};

#[derive(Clone, PartialEq)]
enum Strategia {
    NomeFile,
    JsonPhotoTaken, // photoTakenTime dal JSON
    JsonCreation,   // creationTime dal JSON
    ExifAttuale,
    NomeFilePreferito,
    JsonPreferito,  // Preferisci photoTakenTime, altrimenti creationTime
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
            Strategia::NomeFile => "Usa anno dal nome file",
            Strategia::JsonPhotoTaken => "Usa photoTakenTime dal JSON",
            Strategia::JsonCreation => "Usa creationTime dal JSON",
            Strategia::ExifAttuale => "Mantieni EXIF attuale",
            Strategia::NomeFilePreferito => "Preferisci nome file, altrimenti JSON",
            Strategia::JsonPreferito => "Preferisci photoTakenTime JSON, altrimenti nome file",
        }
    }
}

pub struct CorrectorApp {
    directory: Option<PathBuf>,
    foto_list: Vec<FotoData>,
    foto_selezionate: std::collections::HashSet<usize>, // Indici delle foto selezionate
    ultimo_indice_selezionato: Option<usize>, // Per gestire Shift+click
    strategia_datetime_original: Strategia,
    strategia_create_date: Strategia,
    strategia_modify_date: Strategia,
    #[allow(dead_code)]
    loading: bool,
    #[allow(dead_code)]
    loading_message: String,
    stats: String,
    // Stato per dialog di conferma
    mostra_conferma: bool,
    foto_da_modificare_count: usize, // Numero di foto da modificare per il popup di conferma
    // Stato per barra di avanzamento
    applicando_modifiche: bool,
    foto_totali_da_modificare: usize,
    foto_modificate: usize,
    errori_applicazione: usize,
    // Contatori condivisi per il progresso (usati dal thread di scrittura)
    progresso_counter: Option<std::sync::Arc<std::sync::Mutex<(usize, usize)>>>,
    // Filtro per gravità incongruenza
    soglia_gravita_giorni: f32,
    unita_gravita: UnitaGravita,
    mostra_tutte_foto: bool, // Flag per mostrare tutte le foto, anche senza incongruenze
    // Ordinamento
    colonna_ordinamento: Option<ColonnaOrdinamento>,
    ordine_crescente: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum ColonnaOrdinamento {
    NomeFile,
    Gravita,
    Incongruenze,
    DateTimeOriginal,
    CreateDate,
    ModifyDate,
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

impl UnitaGravita {
    fn display_name(&self) -> &str {
        match self {
            UnitaGravita::Secondi => "Secondi",
            UnitaGravita::Minuti => "Minuti",
            UnitaGravita::Ore => "Ore",
            UnitaGravita::Giorni => "Giorni",
            UnitaGravita::Mesi => "Mesi",
            UnitaGravita::Anni => "Anni",
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
        Self {
            directory: None,
            foto_list: Vec::new(),
            foto_selezionate: std::collections::HashSet::new(),
            ultimo_indice_selezionato: None,
            strategia_datetime_original: Strategia::JsonPhotoTaken,
            strategia_create_date: Strategia::JsonPhotoTaken,
            strategia_modify_date: Strategia::JsonPhotoTaken,
            loading: false,
            loading_message: String::new(),
            stats: String::new(),
            mostra_conferma: false,
            foto_da_modificare_count: 0,
            applicando_modifiche: false,
            foto_totali_da_modificare: 0,
            foto_modificate: 0,
            errori_applicazione: 0,
            progresso_counter: None,
            soglia_gravita_giorni: 0.0,
            unita_gravita: UnitaGravita::Giorni,
            mostra_tutte_foto: false,
            colonna_ordinamento: None,
            ordine_crescente: true,
        }
    }
    
    fn seleziona_cartella(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.directory = Some(path);
            self.carica_foto();
        }
    }
    
    fn carica_foto(&mut self) {
        if let Some(ref dir) = self.directory {
            // Carica le foto (veloce con Rust!)
            self.foto_list = leggi_foto_da_directory(dir);
            self.aggiorna_statistiche();
        }
    }
    
    fn calcola_proposte(&mut self) {
        for foto in &mut self.foto_list {
            foto.strategia_datetime_original = self.strategia_datetime_original.as_str().to_string();
            foto.strategia_create_date = self.strategia_create_date.as_str().to_string();
            foto.strategia_modify_date = self.strategia_modify_date.as_str().to_string();
            
            foto.proposta_datetime_original = crate::calcola_proposta_con_strategia(foto, &foto.strategia_datetime_original);
            foto.proposta_create_date = crate::calcola_proposta_con_strategia(foto, &foto.strategia_create_date);
            foto.proposta_modify_date = crate::calcola_proposta_con_strategia(foto, &foto.strategia_modify_date);
            
            // Ricalcola incongruenze e gravità dopo aver calcolato le proposte
            foto.incongruenze = crate::rileva_incongruenze(foto);
            foto.gravita_incongruenza = crate::calcola_gravita_incongruenza(foto);
        }
    }
    
    fn avvia_applicazione_modifiche(&mut self, _ctx: &egui::Context) {
        // Applica modifiche solo alle foto selezionate
        let foto_da_modificare: Vec<_> = self.foto_list
            .iter()
            .enumerate()
            .filter(|(idx, f)| {
                // Deve essere selezionata E avere almeno una proposta
                self.foto_selezionate.contains(idx) && (
                    f.proposta_datetime_original.is_some() ||
                    f.proposta_create_date.is_some() ||
                    f.proposta_modify_date.is_some()
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
            if let Some(data) = foto.proposta_modify_date {
                campi_da_scrivere.push(("ModifyDate", data));
            }
            
            (foto.path.clone(), campi_da_scrivere)
        }).collect();
        
        // Usa contatori condivisi per comunicare il progresso
        use std::sync::{Arc, Mutex};
        let progresso = Arc::new(Mutex::new((0usize, 0usize))); // (successi, errori)
        self.progresso_counter = Some(progresso.clone());
        
        // Avvia la scrittura in un thread separato
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
                    
                    // Aggiorna contatori condivisi
                    let mut counter = progresso.lock().unwrap();
                    if risultato.is_ok() {
                        counter.0 += 1;
                    } else {
                        counter.1 += 1;
                    }
                    
                    risultato
                })
                .collect();
            
            // Marca come completato impostando un valore speciale
            // (useremo foto_totali_da_modificare + 1 come indicatore di completamento)
        });
    }
    
    fn aggiorna_progresso_da_counter(&mut self, ctx: &egui::Context) {
        let mut completato = false;
        
        if let Some(ref counter_arc) = self.progresso_counter {
            if let Ok(counter) = counter_arc.try_lock() {
                let (successi, errori) = *counter;
                let totale_elaborate = successi + errori;
                
                // Aggiorna lo stato
                self.foto_modificate = successi;
                self.errori_applicazione = errori;
                
                // Se tutte le foto sono state elaborate, completa
                if totale_elaborate >= self.foto_totali_da_modificare && self.foto_totali_da_modificare > 0 {
                    completato = true;
                } else {
                    ctx.request_repaint();
                }
            }
        }
        
        // Rilascia il borrow prima di modificare self
        if completato {
            self.applicando_modifiche = false;
            self.progresso_counter = None;
            
            // Rileggi le foto dopo le modifiche
            if let Some(ref dir) = self.directory {
                // Salva i path delle foto selezionate prima di ricaricare
                let vecchie_selezioni: Vec<_> = self.foto_selezionate.iter()
                    .filter_map(|idx| self.foto_list.get(*idx).map(|f| f.path.clone()))
                    .collect();
                
                self.foto_list = leggi_foto_da_directory(dir);
                
                // Ripristina le selezioni basate sul path
                self.foto_selezionate.clear();
                for (idx, foto) in self.foto_list.iter().enumerate() {
                    if vecchie_selezioni.contains(&foto.path) {
                        self.foto_selezionate.insert(idx);
                    }
                }
                
                self.aggiorna_statistiche();
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
            "Totale foto: {}\nCon EXIF: {}\nCon proposte: {}",
            totale, con_exif, con_proposte
        );
    }
}

impl eframe::App for CorrectorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Dialog di conferma
        if self.mostra_conferma {
            egui::Window::new("⚠️ Conferma Modifiche EXIF")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading("Attenzione!");
                    ui.separator();
                    ui.label("Stai per modificare i metadati EXIF delle tue foto.");
                    ui.label("Questa operazione modifica permanentemente i file.");
                    ui.label("");
                    
                    ui.label(format!("Foto da modificare: {}", self.foto_da_modificare_count));
                    ui.label("");
                    ui.label("⚠️ Assicurati di avere un backup delle foto!");
                    ui.separator();
                    
                    ui.horizontal(|ui| {
                        if ui.button("✅ Conferma e Applica").clicked() {
                            self.mostra_conferma = false;
                            self.avvia_applicazione_modifiche(ctx);
                        }
                        if ui.button("❌ Annulla").clicked() {
                            self.mostra_conferma = false;
                        }
                    });
                });
        }
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Correttore Date EXIF Foto - Versione Rust");
            
            ui.separator();
            
            // Fase 1: Selezione cartella
            ui.horizontal(|ui| {
                ui.label("Fase 1: Leggi Cartella");
                if ui.button("Seleziona Cartella").clicked() {
                    self.seleziona_cartella();
                }
                if let Some(ref dir) = self.directory {
                    ui.label(format!("Cartella: {}", dir.file_name().unwrap_or_default().to_string_lossy()));
                } else {
                    ui.label("Nessuna cartella selezionata");
                }
            });
            
            ui.separator();
            
            // Filtro per gravità con slider che si adatta all'unità selezionata
            ui.horizontal(|ui| {
                ui.label("Filtra per gravità minima:");
                
                // Calcola il valore corrente nell'unità selezionata
                // La soglia è memorizzata in secondi per permettere precisione fino ai secondi
                let mut valore_unita = match self.unita_gravita {
                    UnitaGravita::Secondi => self.soglia_gravita_giorni * 86400.0,
                    UnitaGravita::Minuti => self.soglia_gravita_giorni * 1440.0,
                    UnitaGravita::Ore => self.soglia_gravita_giorni * 24.0,
                    UnitaGravita::Giorni => self.soglia_gravita_giorni,
                    UnitaGravita::Mesi => self.soglia_gravita_giorni / 30.0,
                    UnitaGravita::Anni => self.soglia_gravita_giorni / 365.0,
                };
                
                // Calcola il range dello slider in base all'unità
                // Il minimo è sempre 0 (che corrisponde a 0 secondi)
                let (min_val, max_val) = match self.unita_gravita {
                    UnitaGravita::Secondi => (0.0, 31536000.0), // 0 - 1 anno in secondi
                    UnitaGravita::Minuti => (0.0, 525600.0), // 0 - 1 anno in minuti
                    UnitaGravita::Ore => (0.0, 8760.0), // 0 - 1 anno in ore
                    UnitaGravita::Giorni => (0.0, 3650.0), // 0 - 10 anni (ma può essere convertito in secondi)
                    UnitaGravita::Mesi => (0.0, 120.0), // 0 - 10 anni in mesi
                    UnitaGravita::Anni => (0.0, 10.0), // 0 - 10 anni
                };
                
                // Assicurati che il valore minimo sia almeno 0 (0 secondi)
                valore_unita = valore_unita.max(0.0);
                
                let unita_text = self.unita_gravita.display_name().to_lowercase();
                // Crea il testo dello slider usando il valore corrente (prima del borrow mutabile)
                let slider_text = format!("{:.2} {}", valore_unita, unita_text);
                ui.add(egui::Slider::new(&mut valore_unita, min_val..=max_val)
                    .text(slider_text));
                
                // Converti il valore dell'unità selezionata in giorni (ma mantiene precisione fino ai secondi)
                // Il valore viene convertito in giorni, ma può rappresentare frazioni di secondo
                let nuovo_valore_giorni = self.unita_gravita.to_giorni(valore_unita);
                // Assicurati che non sia negativo (minimo 0 secondi = 0 giorni)
                self.soglia_gravita_giorni = nuovo_valore_giorni.max(0.0);
            });
            
            ui.horizontal(|ui| {
                ui.label("Unità:");
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
            });
            
            ui.separator();
            
            // Checkbox per mostrare tutte le foto
            ui.checkbox(&mut self.mostra_tutte_foto, "Mostra tutte le foto (anche senza incongruenze)");
            
            ui.separator();
            
            // Tabella foto - filtra in base al flag e alla soglia
            let soglia_secondi = (self.soglia_gravita_giorni * 86400.0) as i64;
            let mut foto_da_mostrare: Vec<_> = if self.mostra_tutte_foto {
                // Mostra tutte le foto, ma filtra per soglia se ci sono incongruenze
                self.foto_list
                    .iter()
                    .filter(|f| {
                        if f.incongruenze.is_empty() {
                            return true; // Mostra anche quelle senza incongruenze
                        }
                        // Per quelle con incongruenze, applica il filtro sulla soglia
                        let gravita_secondi = (f.gravita_incongruenza as f64 * 86400.0) as i64;
                        gravita_secondi >= soglia_secondi
                    })
                    .cloned()
                    .collect()
            } else {
                // Mostra solo quelle con incongruenze che superano la soglia
                self.foto_list
                    .iter()
                    .filter(|f| {
                        if f.incongruenze.is_empty() {
                            return false;
                        }
                        // Converti la gravità della foto in secondi per il confronto
                        let gravita_secondi = (f.gravita_incongruenza as f64 * 86400.0) as i64;
                        gravita_secondi >= soglia_secondi
                    })
                    .cloned()
                    .collect()
            };
            
            // Applica ordinamento se selezionato
            if let Some(colonna) = self.colonna_ordinamento {
                foto_da_mostrare.sort_by(|a, b| {
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
                        ColonnaOrdinamento::ModifyDate => {
                            match (a.exif_modify_date, b.exif_modify_date) {
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
            
            // Mostra la soglia nell'unità selezionata
            let soglia_display = match self.unita_gravita {
                UnitaGravita::Secondi => format!("{} secondi", soglia_secondi),
                UnitaGravita::Minuti => format!("{:.2} minuti", self.soglia_gravita_giorni * 1440.0),
                UnitaGravita::Ore => format!("{:.2} ore", self.soglia_gravita_giorni * 24.0),
                UnitaGravita::Giorni => format!("{:.2} giorni", self.soglia_gravita_giorni),
                UnitaGravita::Mesi => format!("{:.2} mesi", self.soglia_gravita_giorni / 30.0),
                UnitaGravita::Anni => format!("{:.2} anni", self.soglia_gravita_giorni / 365.0),
            };
            
            if self.mostra_tutte_foto {
                ui.label(format!("Foto mostrate: {} (tutte, filtro >= {} per incongruenze)", foto_da_mostrare.len(), soglia_display));
            } else {
                ui.label(format!("Foto con incongruenze >= {}: {}", soglia_display, foto_da_mostrare.len()));
            }
            
            ui.label(format!("Foto selezionate: {}", self.foto_selezionate.len()));
            ui.separator();
            
            egui::ScrollArea::both().show(ui, |ui| {
                egui::Grid::new("foto_grid")
                    .num_columns(10)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        // Header con checkbox "Seleziona tutte"
                        let tutte_selezionate = !foto_da_mostrare.is_empty() && 
                            foto_da_mostrare.iter().all(|foto| {
                                let idx_originale = self.foto_list.iter()
                                    .position(|f| f.path == foto.path)
                                    .unwrap_or(0);
                                self.foto_selezionate.contains(&idx_originale)
                            });
                        let mut seleziona_tutte = tutte_selezionate;
                        if ui.checkbox(&mut seleziona_tutte, "Seleziona").changed() {
                            // Seleziona/deseleziona tutte le foto visibili
                            for foto in foto_da_mostrare.iter() {
                                let idx_originale = self.foto_list.iter()
                                    .position(|f| f.path == foto.path)
                                    .unwrap_or(0);
                                if seleziona_tutte {
                                    self.foto_selezionate.insert(idx_originale);
                                } else {
                                    self.foto_selezionate.remove(&idx_originale);
                                }
                            }
                        }
                        ui.label("Nome File");
                        ui.label("Gravità");
                        ui.label("Incongruenze");
                        ui.label("DateTimeOriginal ⭐");
                        ui.label("→ Proposta");
                        ui.label("CreateDate");
                        ui.label("→ Proposta");
                        ui.label("ModifyDate");
                        ui.label("→ Proposta");
                        ui.end_row();
                        
                        // Righe dati - foto filtrate
                        // Crea una lista di indici per gestire Shift+click
                        let indici_visibili: Vec<usize> = foto_da_mostrare.iter()
                            .map(|foto| {
                                self.foto_list.iter()
                                    .position(|f| f.path == foto.path)
                                    .unwrap_or(0)
                            })
                            .collect();
                        
                        for (idx_grid, foto) in foto_da_mostrare.iter().enumerate() {
                            // Trova l'indice originale nella lista completa
                            let idx_originale = self.foto_list.iter()
                                .position(|f| f.path == foto.path)
                                .unwrap_or(0);
                            
                            let mut is_selected = self.foto_selezionate.contains(&idx_originale);
                            
                            // Checkbox per selezione con gestione Ctrl/Shift
                            let checkbox_response = ui.checkbox(&mut is_selected, "");
                            
                            if checkbox_response.changed() {
                                let input = ui.input(|i| i.clone());
                                let ctrl_pressed = input.modifiers.ctrl;
                                let shift_pressed = input.modifiers.shift;
                                
                                if shift_pressed && self.ultimo_indice_selezionato.is_some() {
                                    // Shift+click: seleziona range
                                    let ultimo_idx = self.ultimo_indice_selezionato.unwrap();
                                    let start_idx = indici_visibili.iter().position(|&i| i == ultimo_idx).unwrap_or(0);
                                    let end_idx = idx_grid;
                                    let (start, end) = if start_idx < end_idx {
                                        (start_idx, end_idx)
                                    } else {
                                        (end_idx, start_idx)
                                    };
                                    
                                    for i in start..=end {
                                        if let Some(&idx) = indici_visibili.get(i) {
                                            self.foto_selezionate.insert(idx);
                                        }
                                    }
                                    self.ultimo_indice_selezionato = Some(idx_originale);
                                } else if ctrl_pressed {
                                    // Ctrl+click: aggiungi/rimuovi singola foto
                                    if is_selected {
                                        self.foto_selezionate.insert(idx_originale);
                                    } else {
                                        self.foto_selezionate.remove(&idx_originale);
                                    }
                                    self.ultimo_indice_selezionato = Some(idx_originale);
                                } else {
                                    // Click normale: seleziona solo questa foto
                                    self.foto_selezionate.clear();
                                    self.foto_selezionate.insert(idx_originale);
                                    self.ultimo_indice_selezionato = Some(idx_originale);
                                }
                            } else if is_selected {
                                // Mantieni lo stato selezionato
                                self.foto_selezionate.insert(idx_originale);
                            } else {
                                self.foto_selezionate.remove(&idx_originale);
                            }
                            
                            // Evidenzia riga se selezionata
                            if is_selected {
                                ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(100, 150, 255));
                            }
                            
                            // Nome file - rendilo cliccabile per doppio click
                            let nome_response = ui.selectable_label(false, &foto.nome_file);
                            
                            // Gestione doppio click sul nome file per aprire la foto
                            if nome_response.double_clicked() {
                                let foto_path = foto.path.clone();
                                std::thread::spawn(move || {
                                    let _ = std::process::Command::new("xdg-open")
                                        .arg(&foto_path)
                                        .spawn();
                                });
                            }
                            
                            // Gravità con scala termometrica
                            let giorni_diff = foto.gravita_incongruenza;
                            let gravita_text = if foto.incongruenze.is_empty() {
                                "Nessuna".to_string()
                            } else if giorni_diff == 0 {
                                "OK".to_string()
                            } else if giorni_diff < 30 {
                                format!("{} giorni", giorni_diff)
                            } else if giorni_diff < 365 {
                                format!("{} mesi", giorni_diff / 30)
                            } else {
                                format!("{} anni", giorni_diff / 365)
                            };
                            
                            // Calcola colore termometrico (verde -> giallo -> rosso)
                            let colore = if foto.incongruenze.is_empty() {
                                egui::Color32::from_rgb(150, 150, 150) // Grigio per nessuna incongruenza
                            } else if giorni_diff == 0 {
                                egui::Color32::from_rgb(0, 200, 0) // Verde
                            } else if giorni_diff < 30 {
                                egui::Color32::from_rgb(100, 200, 0) // Verde-giallo
                            } else if giorni_diff < 90 {
                                egui::Color32::from_rgb(200, 200, 0) // Giallo
                            } else if giorni_diff < 365 {
                                egui::Color32::from_rgb(255, 150, 0) // Arancione
                            } else {
                                egui::Color32::from_rgb(255, 0, 0) // Rosso
                            };
                            
                            ui.visuals_mut().override_text_color = Some(colore);
                            ui.label(gravita_text);
                            ui.visuals_mut().override_text_color = None;
                            
                            // Incongruenze - mostra "Nessuna" se non ci sono, altrimenti rendilo cliccabile
                            let inc_text = if foto.incongruenze.is_empty() {
                                "Nessuna".to_string()
                            } else {
                                foto.incongruenze.join("; ")
                            };
                            
                            // Se ci sono incongruenze, rendi il testo cliccabile per aprire il JSON
                            if !foto.incongruenze.is_empty() {
                                let inc_response = ui.selectable_label(false, &inc_text);
                                if inc_response.clicked() {
                                    // Trova il file JSON corrispondente
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
                            
                            // DateTimeOriginal attuale
                            if let Some(dt) = foto.exif_datetime_original {
                                ui.label(dt.format("%Y-%m-%d %H:%M:%S").to_string());
                            } else {
                                ui.label("❌");
                            }
                            
                            // DateTimeOriginal proposta
                            if let Some(dt) = foto.proposta_datetime_original {
                                ui.label(format!("→ {}", dt.format("%Y-%m-%d %H:%M:%S")));
                            } else {
                                ui.label("-");
                            }
                            
                            // CreateDate attuale
                            if let Some(dt) = foto.exif_create_date {
                                ui.label(dt.format("%Y-%m-%d %H:%M:%S").to_string());
                            } else {
                                ui.label("❌");
                            }
                            
                            // CreateDate proposta
                            if let Some(dt) = foto.proposta_create_date {
                                ui.label(format!("→ {}", dt.format("%Y-%m-%d %H:%M:%S")));
                            } else {
                                ui.label("-");
                            }
                            
                            // ModifyDate attuale
                            if let Some(dt) = foto.exif_modify_date {
                                ui.label(dt.format("%Y-%m-%d %H:%M:%S").to_string());
                            } else {
                                ui.label("❌");
                            }
                            
                            // ModifyDate proposta
                            if let Some(dt) = foto.proposta_modify_date {
                                ui.label(format!("→ {}", dt.format("%Y-%m-%d %H:%M:%S")));
                            } else {
                                ui.label("-");
                            }
                            
                            // Reset colore alla fine della riga
                            ui.visuals_mut().override_text_color = None;
                            ui.end_row();
                        }
                    });
            });
        });
        
        // Pannello laterale destro
        egui::SidePanel::right("controlli").show(ctx, |ui| {
            ui.heading("Controlli");
            
            ui.separator();
            
            // Fase 2: Proposta modifiche
            ui.group(|ui| {
                ui.label("Fase 2: Proposta Modifiche");
                ui.separator();
                
                let mut strategia_cambiata = false;
                
                ui.label("Strategia DateTimeOriginal ⭐:");
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
                
                ui.label("Strategia CreateDate:");
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
                
                ui.label("Strategia ModifyDate:");
                egui::ComboBox::from_id_source("strategia_md")
                    .selected_text(self.strategia_modify_date.display_name())
                    .show_ui(ui, |ui| {
                        for strategia in [
                            Strategia::JsonPhotoTaken,
                            Strategia::JsonCreation,
                            Strategia::NomeFilePreferito,
                            Strategia::NomeFile,
                            Strategia::JsonPreferito,
                            Strategia::ExifAttuale,
                        ] {
                            if ui.selectable_value(&mut self.strategia_modify_date, strategia.clone(), strategia.display_name()).changed() {
                                strategia_cambiata = true;
                            }
                        }
                    });
                
                ui.separator();
                
                // Se una qualsiasi strategia è cambiata, ricalcola automaticamente le proposte
                if strategia_cambiata {
                    self.calcola_proposte();
                }
                
                ui.label("Le proposte vengono ricalcolate automaticamente quando cambi le strategie.");
            });
            
            ui.separator();
            
            // Fase 3: Applica modifiche
            ui.group(|ui| {
                ui.label("Fase 3: Applica Modifiche");
                ui.separator();
                
                ui.label("Le modifiche verranno applicate solo alle foto selezionate.");
                ui.label("Ogni campo EXIF può avere una strategia indipendente.");
                
                let foto_selezionate_count = self.foto_selezionate.len();
                if foto_selezionate_count == 0 {
                    ui.label("⚠️ Nessuna foto selezionata!");
                } else {
                    ui.label(format!("✅ {} foto selezionate", foto_selezionate_count));
                }
                
                if ui.button("Applica Modifiche").clicked() {
                    if foto_selezionate_count > 0 {
                        // Calcola quante foto selezionate hanno effettivamente proposte da applicare
                        let foto_con_proposte = self.foto_list
                            .iter()
                            .enumerate()
                            .filter(|(idx, f)| {
                                self.foto_selezionate.contains(idx) && (
                                    f.proposta_datetime_original.is_some() ||
                                    f.proposta_create_date.is_some() ||
                                    f.proposta_modify_date.is_some()
                                )
                            })
                            .count();
                        
                        if foto_con_proposte > 0 {
                            self.foto_da_modificare_count = foto_con_proposte;
                            self.mostra_conferma = true;
                        }
                    }
                }
                
                // Mostra barra di avanzamento se sta applicando modifiche
                if self.applicando_modifiche {
                    ui.separator();
                    ui.label("Applicazione modifiche in corso...");
                    let progresso = if self.foto_totali_da_modificare > 0 {
                        self.foto_modificate as f32 / self.foto_totali_da_modificare as f32
                    } else {
                        0.0
                    };
                    ui.add(egui::ProgressBar::new(progresso).show_percentage());
                    ui.label(format!("{}/{} foto elaborate", self.foto_modificate, self.foto_totali_da_modificare));
                    
                    // Aggiorna progresso dal counter condiviso
                    self.aggiorna_progresso_da_counter(ctx);
                } else if self.foto_modificate > 0 {
                    ui.separator();
                    ui.label(format!("✅ Completato: {} foto modificate", self.foto_modificate));
                    if self.errori_applicazione > 0 {
                        ui.label(format!("⚠️ Errori: {}", self.errori_applicazione));
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

