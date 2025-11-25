use eframe::egui;
use std::path::PathBuf;
use crate::{FotoData, leggi_foto_da_directory, calcola_proposta, scrivi_exif_datetime};

#[derive(Clone, PartialEq)]
enum Strategia {
    NomeFile,
    Json,
    ExifAttuale,
    NomeFilePreferito,
    JsonPreferito,
}

impl Strategia {
    fn as_str(&self) -> &str {
        match self {
            Strategia::NomeFile => "nome_file",
            Strategia::Json => "json",
            Strategia::ExifAttuale => "exif_attuale",
            Strategia::NomeFilePreferito => "nome_file_preferito",
            Strategia::JsonPreferito => "json_preferito",
        }
    }
    
    fn from_str(s: &str) -> Self {
        match s {
            "nome_file" => Strategia::NomeFile,
            "json" => Strategia::Json,
            "exif_attuale" => Strategia::ExifAttuale,
            "nome_file_preferito" => Strategia::NomeFilePreferito,
            "json_preferito" => Strategia::JsonPreferito,
            _ => Strategia::NomeFilePreferito,
        }
    }
    
    fn display_name(&self) -> &str {
        match self {
            Strategia::NomeFile => "Usa anno dal nome file",
            Strategia::Json => "Usa data dal JSON",
            Strategia::ExifAttuale => "Mantieni EXIF attuale",
            Strategia::NomeFilePreferito => "Preferisci nome file, altrimenti JSON",
            Strategia::JsonPreferito => "Preferisci JSON, altrimenti nome file",
        }
    }
}

pub struct CorrectorApp {
    directory: Option<PathBuf>,
    foto_list: Vec<FotoData>,
    strategia_globale: Strategia,
    solo_datetime: bool,
    #[allow(dead_code)]
    loading: bool,
    #[allow(dead_code)]
    loading_message: String,
    stats: String,
}

impl CorrectorApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            directory: None,
            foto_list: Vec::new(),
            strategia_globale: Strategia::NomeFilePreferito,
            solo_datetime: true,
            loading: false,
            loading_message: String::new(),
            stats: String::new(),
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
            foto.strategia = self.strategia_globale.as_str().to_string();
            foto.proposta_datetime_original = calcola_proposta(foto);
        }
    }
    
    fn applica_modifiche(&mut self) {
        let foto_da_modificare: Vec<_> = self.foto_list
            .iter()
            .filter(|f| f.proposta_datetime_original.is_some())
            .cloned()
            .collect();
        
        if foto_da_modificare.is_empty() {
            return;
        }
        
        let solo_dt = self.solo_datetime;
        let mut _successi = 0;
        let mut _errori = 0;
        
        for foto in &foto_da_modificare {
            if let Some(data) = foto.proposta_datetime_original {
                if scrivi_exif_datetime(&foto.path, data, solo_dt).is_ok() {
                    _successi += 1;
                } else {
                    _errori += 1;
                }
            }
        }
        
        // TODO: Mostrare messaggio di successo/errore all'utente nella GUI
        
        // Rileggi le foto dopo le modifiche
        if let Some(ref dir) = self.directory {
            self.foto_list = leggi_foto_da_directory(dir);
            self.aggiorna_statistiche();
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
            
            // Tabella foto
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("foto_grid")
                    .num_columns(5)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        // Header
                        ui.label("Nome File");
                        ui.label("DateTimeOriginal ⭐");
                        ui.label("CreateDate");
                        ui.label("ModifyDate");
                        ui.label("Strategia");
                        ui.end_row();
                        
                        // Righe dati
                        for foto in &self.foto_list {
                            let has_proposta = foto.proposta_datetime_original.is_some();
                            
                            if has_proposta {
                                ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(200, 150, 0));
                            }
                            
                            ui.label(&foto.nome_file);
                            
                            if let Some(dt) = foto.exif_datetime_original {
                                if has_proposta {
                                    ui.label(format!("→ {}", dt.format("%Y-%m-%d %H:%M:%S")));
                                } else {
                                    ui.label(dt.format("%Y-%m-%d %H:%M:%S").to_string());
                                }
                            } else {
                                ui.label("❌");
                            }
                            
                            if let Some(dt) = foto.exif_create_date {
                                ui.label(dt.format("%Y-%m-%d %H:%M:%S").to_string());
                            } else {
                                ui.label("❌");
                            }
                            
                            if let Some(dt) = foto.exif_modify_date {
                                ui.label(dt.format("%Y-%m-%d %H:%M:%S").to_string());
                            } else {
                                ui.label("❌");
                            }
                            
                            ui.label(Strategia::from_str(&foto.strategia).display_name());
                            
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
                
                ui.label("Strategia globale:");
                egui::ComboBox::from_id_source("strategia")
                    .selected_text(self.strategia_globale.display_name())
                    .show_ui(ui, |ui| {
                        for strategia in [
                            Strategia::NomeFilePreferito,
                            Strategia::NomeFile,
                            Strategia::Json,
                            Strategia::JsonPreferito,
                            Strategia::ExifAttuale,
                        ] {
                            ui.selectable_value(&mut self.strategia_globale, strategia.clone(), strategia.display_name());
                        }
                    });
                
                if ui.button("Calcola Proposte").clicked() {
                    self.calcola_proposte();
                }
            });
            
            ui.separator();
            
            // Fase 3: Applica modifiche
            ui.group(|ui| {
                ui.label("Fase 3: Applica Modifiche");
                ui.separator();
                
                ui.checkbox(&mut self.solo_datetime, "Solo DateTimeOriginal (consigliato)");
                
                if ui.button("Applica Modifiche").clicked() {
                    self.applica_modifiche();
                }
            });
            
            ui.separator();
            
            // Statistiche
            ui.label("Statistiche:");
            ui.label(&self.stats);
        });
    }
}

