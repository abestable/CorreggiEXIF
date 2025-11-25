use eframe::egui;
use std::path::PathBuf;
use crate::{FotoData, leggi_foto_da_directory, scrivi_tutti_campi_exif};

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
    
    #[allow(dead_code)]
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
    // Stato per barra di avanzamento
    applicando_modifiche: bool,
    foto_totali_da_modificare: usize,
    foto_modificate: usize,
    errori_applicazione: usize,
    // Contatori condivisi per il progresso (usati dal thread di scrittura)
    progresso_counter: Option<std::sync::Arc<std::sync::Mutex<(usize, usize)>>>,
}

impl CorrectorApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            directory: None,
            foto_list: Vec::new(),
            strategia_datetime_original: Strategia::NomeFilePreferito,
            strategia_create_date: Strategia::NomeFilePreferito,
            strategia_modify_date: Strategia::NomeFilePreferito,
            loading: false,
            loading_message: String::new(),
            stats: String::new(),
            mostra_conferma: false,
            applicando_modifiche: false,
            foto_totali_da_modificare: 0,
            foto_modificate: 0,
            errori_applicazione: 0,
            progresso_counter: None,
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
        }
    }
    
    fn avvia_applicazione_modifiche(&mut self, ctx: &egui::Context) {
        let foto_da_modificare: Vec<_> = self.foto_list
            .iter()
            .filter(|f| {
                f.proposta_datetime_original.is_some() ||
                f.proposta_create_date.is_some() ||
                f.proposta_modify_date.is_some()
            })
            .cloned()
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
        
        let directory_clone = self.directory.clone();
        
        // Avvia la scrittura in un thread separato
        std::thread::spawn(move || {
            use rayon::prelude::*;
            
            let risultati: Vec<_> = dati_scrittura
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
        if let Some(ref counter_arc) = self.progresso_counter {
            if let Ok(counter) = counter_arc.try_lock() {
                let (successi, errori) = *counter;
                let totale_elaborate = successi + errori;
                
                // Aggiorna lo stato
                self.foto_modificate = successi;
                self.errori_applicazione = errori;
                
                // Se tutte le foto sono state elaborate, completa
                if totale_elaborate >= self.foto_totali_da_modificare && self.foto_totali_da_modificare > 0 {
                    self.applicando_modifiche = false;
                    self.progresso_counter = None;
                    
                    // Rileggi le foto dopo le modifiche
                    if let Some(ref dir) = self.directory {
                        self.foto_list = leggi_foto_da_directory(dir);
                        self.aggiorna_statistiche();
                    }
                } else {
                    ctx.request_repaint();
                }
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
                    
                    let foto_da_modificare = self.foto_list
                        .iter()
                        .filter(|f| {
                            f.proposta_datetime_original.is_some() ||
                            f.proposta_create_date.is_some() ||
                            f.proposta_modify_date.is_some()
                        })
                        .count();
                    
                    ui.label(format!("Foto da modificare: {}", foto_da_modificare));
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
            
            // Tabella foto
            egui::ScrollArea::both().show(ui, |ui| {
                egui::Grid::new("foto_grid")
                    .num_columns(7)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        // Header
                        ui.label("Nome File");
                        ui.label("DateTimeOriginal ⭐");
                        ui.label("→ Proposta");
                        ui.label("CreateDate");
                        ui.label("→ Proposta");
                        ui.label("ModifyDate");
                        ui.label("→ Proposta");
                        ui.end_row();
                        
                        // Righe dati
                        for foto in &self.foto_list {
                            let has_proposta = foto.proposta_datetime_original.is_some();
                            
                            // Evidenzia riga se ha proposte
                            if has_proposta {
                                ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(200, 150, 0));
                            }
                            
                            ui.label(&foto.nome_file);
                            
                            // DateTimeOriginal attuale
                            if let Some(dt) = foto.exif_datetime_original {
                                ui.label(dt.format("%Y-%m-%d %H:%M:%S").to_string());
                            } else {
                                ui.label("❌");
                            }
                            
                            // DateTimeOriginal proposta
                            if let Some(dt) = foto.proposta_datetime_original {
                                ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(0, 200, 0));
                                ui.label(format!("→ {}", dt.format("%Y-%m-%d %H:%M:%S")));
                                ui.visuals_mut().override_text_color = if has_proposta {
                                    Some(egui::Color32::from_rgb(200, 150, 0))
                                } else {
                                    None
                                };
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
                                ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(0, 200, 0));
                                ui.label(format!("→ {}", dt.format("%Y-%m-%d %H:%M:%S")));
                                ui.visuals_mut().override_text_color = if has_proposta {
                                    Some(egui::Color32::from_rgb(200, 150, 0))
                                } else {
                                    None
                                };
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
                                ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(0, 200, 0));
                                ui.label(format!("→ {}", dt.format("%Y-%m-%d %H:%M:%S")));
                                ui.visuals_mut().override_text_color = if has_proposta {
                                    Some(egui::Color32::from_rgb(200, 150, 0))
                                } else {
                                    None
                                };
                            } else {
                                ui.label("-");
                            }
                            
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
                let vecchia_strategia_dt = self.strategia_datetime_original.clone();
                egui::ComboBox::from_id_source("strategia_dt")
                    .selected_text(self.strategia_datetime_original.display_name())
                    .show_ui(ui, |ui| {
                        for strategia in [
                            Strategia::NomeFilePreferito,
                            Strategia::NomeFile,
                            Strategia::Json,
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
                let vecchia_strategia_cd = self.strategia_create_date.clone();
                egui::ComboBox::from_id_source("strategia_cd")
                    .selected_text(self.strategia_create_date.display_name())
                    .show_ui(ui, |ui| {
                        for strategia in [
                            Strategia::NomeFilePreferito,
                            Strategia::NomeFile,
                            Strategia::Json,
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
                let vecchia_strategia_md = self.strategia_modify_date.clone();
                egui::ComboBox::from_id_source("strategia_md")
                    .selected_text(self.strategia_modify_date.display_name())
                    .show_ui(ui, |ui| {
                        for strategia in [
                            Strategia::NomeFilePreferito,
                            Strategia::NomeFile,
                            Strategia::Json,
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
                
                if ui.button("Calcola Proposte").clicked() {
                    self.calcola_proposte();
                }
            });
            
            ui.separator();
            
            // Fase 3: Applica modifiche
            ui.group(|ui| {
                ui.label("Fase 3: Applica Modifiche");
                ui.separator();
                
                ui.label("Le modifiche verranno applicate solo ai campi con proposte.");
                ui.label("Ogni campo EXIF può avere una strategia indipendente.");
                
                if ui.button("Applica Modifiche").clicked() {
                    let foto_da_modificare = self.foto_list
                        .iter()
                        .filter(|f| {
                            f.proposta_datetime_original.is_some() ||
                            f.proposta_create_date.is_some() ||
                            f.proposta_modify_date.is_some()
                        })
                        .count();
                    
                    if foto_da_modificare > 0 {
                        self.mostra_conferma = true;
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

