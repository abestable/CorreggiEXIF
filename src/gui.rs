use eframe::egui;
use std::path::PathBuf;
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
    foto_list: Vec<FotoData>,
    foto_selezionate: std::collections::HashSet<usize>, // Indici delle foto selezionate
    ultimo_indice_selezionato: Option<usize>, // Per gestire Shift+click
    strategia_datetime_original: Strategia,
    strategia_create_date: Strategia,
    #[allow(dead_code)]
    loading: bool,
    #[allow(dead_code)]
    loading_message: String,
    stats: String,
    // State for confirmation dialog
    mostra_conferma: bool,
    foto_da_modificare_count: usize, // Number of photos to modify for confirmation popup
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
    // Sorting
    colonna_ordinamento: Option<ColonnaOrdinamento>,
    ordine_crescente: bool,
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
        Self {
            directory: None,
            foto_list: Vec::new(),
            foto_selezionate: std::collections::HashSet::new(),
            ultimo_indice_selezionato: None,
            strategia_datetime_original: Strategia::JsonPhotoTaken,
            strategia_create_date: Strategia::JsonPhotoTaken,
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
            // Ricalcola le proposte con le strategie corrette della GUI
            // (le foto vengono caricate con strategia di default "nome_file_preferito",
            // ma la GUI usa "JsonPhotoTaken" di default)
            self.calcola_proposte();
            self.aggiorna_statistiche();
        }
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
                
                // Restore selections based on path
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
            "Total photos: {}\nWith EXIF: {}\nWith proposals: {}",
            totale, con_exif, con_proposte
        );
    }
}

impl eframe::App for CorrectorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Confirmation dialog
        if self.mostra_conferma {
            egui::Window::new("⚠️ Confirm EXIF Changes")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading("Warning!");
                    ui.separator();
                    ui.label("You are about to modify the EXIF metadata of your photos.");
                    ui.label("This operation permanently modifies the files.");
                    ui.label("");
                    
                    ui.label(format!("Photos to modify: {}", self.foto_da_modificare_count));
                    ui.label("");
                    ui.label("⚠️ Make sure you have a backup of your photos!");
                    ui.separator();
                    
                    ui.horizontal(|ui| {
                        if ui.button("✅ Confirm and Apply").clicked() {
                            self.mostra_conferma = false;
                            self.avvia_applicazione_modifiche(ctx);
                        }
                        if ui.button("❌ Cancel").clicked() {
                            self.mostra_conferma = false;
                        }
                    });
                });
        }
        
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
                let nuovo_valore_giorni = self.unita_gravita.to_giorni(valore_unita);
                // Ensure it's not negative (minimum 0 seconds = 0 days)
                self.soglia_gravita_giorni = nuovo_valore_giorni.max(0.0);
            });
            
            ui.horizontal(|ui| {
                ui.label("Unit:");
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
            
            // Checkbox to show all photos
            ui.checkbox(&mut self.mostra_tutte_foto, "Show all photos (including those without incongruities)");
            
            ui.separator();
            
            // Photo table - filter based on flag and threshold
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
                    };
                    
                    if self.ordine_crescente {
                        cmp
                    } else {
                        cmp.reverse()
                    }
                });
            }
            
            // Show threshold in selected unit
            let soglia_display = match self.unita_gravita {
                UnitaGravita::Secondi => format!("{} seconds", soglia_secondi),
                UnitaGravita::Minuti => format!("{:.2} minutes", self.soglia_gravita_giorni * 1440.0),
                UnitaGravita::Ore => format!("{:.2} hours", self.soglia_gravita_giorni * 24.0),
                UnitaGravita::Giorni => format!("{:.2} days", self.soglia_gravita_giorni),
                UnitaGravita::Mesi => format!("{:.2} months", self.soglia_gravita_giorni / 30.0),
                UnitaGravita::Anni => format!("{:.2} years", self.soglia_gravita_giorni / 365.0),
            };
            
            if self.mostra_tutte_foto {
                ui.label(format!("Photos shown: {} (all, filter >= {} for incongruities)", foto_da_mostrare.len(), soglia_display));
            } else {
                ui.label(format!("Photos with incongruities >= {}: {}", soglia_display, foto_da_mostrare.len()));
            }
            
            ui.label(format!("Selected photos: {}", self.foto_selezionate.len()));
            ui.separator();
            
            egui::ScrollArea::both().show(ui, |ui| {
                egui::Grid::new("foto_grid")
                    .num_columns(8)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        // Header with checkbox "Select all"
                        let tutte_selezionate = !foto_da_mostrare.is_empty() && 
                            foto_da_mostrare.iter().all(|foto| {
                                let idx_originale = self.foto_list.iter()
                                    .position(|f| f.path == foto.path)
                                    .unwrap_or(0);
                                self.foto_selezionate.contains(&idx_originale)
                            });
                        let mut seleziona_tutte = tutte_selezionate;
                        if ui.checkbox(&mut seleziona_tutte, "Select").changed() {
                            // Select/deselect all visible photos
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
                        // Clickable headers for sorting
                        let nome_response = ui.selectable_label(
                            self.colonna_ordinamento == Some(ColonnaOrdinamento::NomeFile),
                            "File Name"
                        );
                        if nome_response.clicked() {
                            if self.colonna_ordinamento == Some(ColonnaOrdinamento::NomeFile) {
                                self.ordine_crescente = !self.ordine_crescente;
                            } else {
                                self.colonna_ordinamento = Some(ColonnaOrdinamento::NomeFile);
                                self.ordine_crescente = true;
                            }
                        }
                        
                        let gravita_response = ui.selectable_label(
                            self.colonna_ordinamento == Some(ColonnaOrdinamento::Gravita),
                            "Severity"
                        );
                        if gravita_response.clicked() {
                            if self.colonna_ordinamento == Some(ColonnaOrdinamento::Gravita) {
                                self.ordine_crescente = !self.ordine_crescente;
                            } else {
                                self.colonna_ordinamento = Some(ColonnaOrdinamento::Gravita);
                                self.ordine_crescente = true;
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
                            } else {
                                self.colonna_ordinamento = Some(ColonnaOrdinamento::Incongruenze);
                                self.ordine_crescente = true;
                            }
                        }
                        
                        let dt_response = ui.selectable_label(
                            self.colonna_ordinamento == Some(ColonnaOrdinamento::DateTimeOriginal),
                            "DateTimeOriginal ⭐"
                        );
                        if dt_response.clicked() {
                            if self.colonna_ordinamento == Some(ColonnaOrdinamento::DateTimeOriginal) {
                                self.ordine_crescente = !self.ordine_crescente;
                            } else {
                                self.colonna_ordinamento = Some(ColonnaOrdinamento::DateTimeOriginal);
                                self.ordine_crescente = true;
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
                            } else {
                                self.colonna_ordinamento = Some(ColonnaOrdinamento::CreateDate);
                                self.ordine_crescente = true;
                            }
                        }
                        
                        ui.label("→ Proposal");
                        ui.end_row();
                        
                        // Data rows - filtered photos
                        // Create a list of indices to handle Shift+click
                        let indici_visibili: Vec<usize> = foto_da_mostrare.iter()
                            .map(|foto| {
                                self.foto_list.iter()
                                    .position(|f| f.path == foto.path)
                                    .unwrap_or(0)
                            })
                            .collect();
                        
                        for (idx_grid, foto) in foto_da_mostrare.iter().enumerate() {
                            // Find original index in complete list
                            let idx_originale = self.foto_list.iter()
                                .position(|f| f.path == foto.path)
                                .unwrap_or(0);
                            
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
                                    // Ctrl+click: add/remove single photo
                                    if is_selected {
                                        self.foto_selezionate.insert(idx_originale);
                                    } else {
                                        self.foto_selezionate.remove(&idx_originale);
                                    }
                                    self.ultimo_indice_selezionato = Some(idx_originale);
                                } else {
                                    // Normal click: select only this photo
                                    self.foto_selezionate.clear();
                                    self.foto_selezionate.insert(idx_originale);
                                    self.ultimo_indice_selezionato = Some(idx_originale);
                                }
                            } else if is_selected {
                                // Keep selected state
                                self.foto_selezionate.insert(idx_originale);
                            } else {
                                self.foto_selezionate.remove(&idx_originale);
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
                            self.mostra_conferma = true;
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

