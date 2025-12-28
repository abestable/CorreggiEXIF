pub mod gui;

use exif::{In, Tag, Value};
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc, NaiveDateTime, Datelike, Timelike};
use rayon::prelude::*;

#[derive(Debug, Clone)]
pub struct FotoData {
    #[allow(dead_code)]
    path: PathBuf,
    nome_file: String,
    #[allow(dead_code)]
    anno_nome: Option<i32>,
    data_nome: Option<(i32, u32, u32)>, // (anno, mese, giorno)
    data_json: Option<DateTime<Utc>>, // photoTakenTime dal JSON
    data_json_creation: Option<DateTime<Utc>>, // creationTime dal JSON
    exif_datetime_original: Option<DateTime<Utc>>,
    exif_create_date: Option<DateTime<Utc>>,
    #[allow(dead_code)]
    exif_modify_date: Option<DateTime<Utc>>,
    proposta_datetime_original: Option<DateTime<Utc>>,
    proposta_create_date: Option<DateTime<Utc>>,
    proposta_modify_date: Option<DateTime<Utc>>,
    strategia_datetime_original: String,
    strategia_create_date: String,
    strategia_modify_date: String,
    incongruenze: Vec<String>, // Lista di incongruenze rilevate
    gravita_incongruenza: i64, // Differenza in giorni (0 = nessuna incongruenza)
}

#[derive(Debug, Deserialize)]
struct GooglePhotoJson {
    #[serde(rename = "photoTakenTime")]
    photo_taken_time: Option<PhotoTime>,
    #[serde(rename = "creationTime")]
    creation_time: Option<PhotoTime>,
}

#[derive(Debug, Deserialize)]
struct PhotoTime {
    timestamp: String,
}

pub fn estrai_anno_da_nome(nome_file: &str) -> Option<(i32, u32, u32)> {
    // Pattern: "2002_" all'inizio
    let re = Regex::new(r"^(\d{4})_").ok()?;
    if let Some(caps) = re.captures(nome_file) {
        if let Ok(anno) = caps[1].parse::<i32>() {
            if (1900..=2100).contains(&anno) {
                return Some((anno, 1, 1));
            }
        }
    }
    
    // Pattern: "20050806" (YYYYMMDD)
    let re = Regex::new(r"(\d{4})(\d{2})(\d{2})").ok()?;
    if let Some(caps) = re.captures(nome_file) {
        if let Ok(anno) = caps[1].parse::<i32>() {
            if let Ok(mese) = caps[2].parse::<u32>() {
                if let Ok(giorno) = caps[3].parse::<u32>() {
                    if (1900..=2100).contains(&anno) && (1..=12).contains(&mese) && (1..=31).contains(&giorno) {
                        return Some((anno, mese, giorno));
                    }
                }
            }
        }
    }
    
    // Pattern: "24082009" (DDMMYYYY)
    let re = Regex::new(r"(\d{2})(\d{2})(\d{4})").ok()?;
    if let Some(caps) = re.captures(nome_file) {
        if let Ok(giorno) = caps[1].parse::<u32>() {
            if let Ok(mese) = caps[2].parse::<u32>() {
                if let Ok(anno) = caps[3].parse::<i64>() {
                    if (1900..=2100).contains(&(anno as i32)) && (1..=12).contains(&mese) && (1..=31).contains(&giorno) {
                        return Some((anno as i32, mese, giorno));
                    }
                }
            }
        }
    }
    
    None
}

pub fn trova_file_json(foto_path: &Path) -> Option<PathBuf> {
    let directory = foto_path.parent()?;
    let base_name = foto_path.file_stem()?.to_str()?;
    let nome_file = foto_path.file_name()?.to_str()?;
    
    let possibili_nomi = vec![
        format!("{}.supplemental-metadata.json", nome_file),
        format!("{}.supplemental-metadata.json", base_name),
        format!("{}.supplemental.json", nome_file),
        format!("{}.supplemental.json", base_name),
        format!("{}.supplemental-met.json", nome_file),
        format!("{}.supplemental-met.json", base_name),
        format!("{}.supplemental-me.json", nome_file),
        format!("{}.supplemental-me.json", base_name),
        format!("{}.supplemental-metad.json", nome_file),
        format!("{}.supplemental-metad.json", base_name),
        format!("{}.supplementa.json", nome_file),
        format!("{}.supplementa.json", base_name),
        format!("{}.supplemen.json", nome_file),
        format!("{}.supplemen.json", base_name),
    ];
    
    for nome_json in possibili_nomi {
        let json_path = directory.join(&nome_json);
        if json_path.exists() {
            return Some(json_path);
        }
    }
    
    None
}

pub fn leggi_data_json(json_path: &Path) -> Option<DateTime<Utc>> {
    let content = fs::read_to_string(json_path).ok()?;
    let json: GooglePhotoJson = serde_json::from_str(&content).ok()?;
    
    // Preferisci photoTakenTime, altrimenti creationTime
    if let Some(photo_time) = json.photo_taken_time {
        if let Ok(timestamp) = photo_time.timestamp.parse::<i64>() {
            return DateTime::from_timestamp(timestamp, 0);
        }
    }
    
    if let Some(creation_time) = json.creation_time {
        if let Ok(timestamp) = creation_time.timestamp.parse::<i64>() {
            return DateTime::from_timestamp(timestamp, 0);
        }
    }
    
    None
}

pub fn leggi_data_json_completo(json_path: &Path) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
    let content = match fs::read_to_string(json_path) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };
    
    let json: GooglePhotoJson = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(_) => return (None, None),
    };
    
    let photo_taken = json.photo_taken_time.and_then(|pt| {
        pt.timestamp.parse::<i64>().ok()
            .and_then(|ts| {
                // Usa from_timestamp che preserva l'ora completa
                DateTime::from_timestamp(ts, 0)
            })
    });
    
    let creation = json.creation_time.and_then(|ct| {
        ct.timestamp.parse::<i64>().ok()
            .and_then(|ts| {
                // Usa from_timestamp che preserva l'ora completa
                DateTime::from_timestamp(ts, 0)
            })
    });
    
    (photo_taken, creation)
}

pub fn leggi_exif_datetime(file_path: &Path, tag: Tag) -> Option<DateTime<Utc>> {
    let file = fs::File::open(file_path).ok()?;
    let mut bufreader = std::io::BufReader::new(&file);
    let exif = exif::Reader::new();
    let exif_data = exif.read_from_container(&mut bufreader).ok()?;
    
    if let Some(field) = exif_data.get_field(tag, In::PRIMARY) {
        if let Value::Ascii(ref vec) = field.value {
            if !vec.is_empty() {
                let date_str = String::from_utf8_lossy(&vec[0]);
                // Formato: "2002:01:01 12:00:00"
                if let Ok(dt) = NaiveDateTime::parse_from_str(&date_str, "%Y:%m:%d %H:%M:%S") {
                    return Some(DateTime::from_naive_utc_and_offset(dt, Utc));
                }
            }
        }
    }
    
    None
}

pub fn ottieni_tutti_campi_exif(foto_path: &Path) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
    let datetime_original = leggi_exif_datetime(foto_path, Tag::DateTimeOriginal);
    let create_date = leggi_exif_datetime(foto_path, Tag::DateTimeDigitized);
    let modify_date = leggi_exif_datetime(foto_path, Tag::DateTime);
    
    (datetime_original, create_date, modify_date)
}

pub fn calcola_proposta_con_strategia(foto: &FotoData, strategia: &str) -> Option<DateTime<Utc>> {
    match strategia {
        "nome_file" => {
            if let Some((anno, mese, giorno)) = foto.data_nome {
                if let Ok(dt) = NaiveDateTime::parse_from_str(
                    &format!("{:04}-{:02}-{:02} 12:00:00", anno, mese, giorno),
                    "%Y-%m-%d %H:%M:%S"
                ) {
                    return Some(DateTime::from_naive_utc_and_offset(dt, Utc));
                }
            }
        }
        "json_photo_taken" => {
            // Restituisce direttamente foto.data_json che contiene l'ora corretta dal JSON
            return foto.data_json;
        }
        "json_creation" => {
            return foto.data_json_creation;
        }
        "exif_attuale" => {
            return foto.exif_datetime_original;
        }
        "nome_file_preferito" => {
            // Questa strategia preferisce il nome file (con ora 12:00:00) al JSON
            // ATTENZIONE: quando c'è un nome file con data, usa sempre 12:00:00 invece dell'ora dal JSON
            if let Some((anno, mese, giorno)) = foto.data_nome {
                if let Ok(dt) = NaiveDateTime::parse_from_str(
                    &format!("{:04}-{:02}-{:02} 12:00:00", anno, mese, giorno),
                    "%Y-%m-%d %H:%M:%S"
                ) {
                    return Some(DateTime::from_naive_utc_and_offset(dt, Utc));
                }
            }
            // Se non c'è nome file, usa il JSON (che contiene l'ora corretta)
            return foto.data_json;
        }
        "json_preferito" => {
            // Preferisci photoTakenTime, altrimenti creationTime, altrimenti nome file
            if let Some(dt) = foto.data_json {
                return Some(dt);
            }
            if let Some(dt) = foto.data_json_creation {
                return Some(dt);
            }
            if let Some((anno, mese, giorno)) = foto.data_nome {
                if let Ok(dt) = NaiveDateTime::parse_from_str(
                    &format!("{:04}-{:02}-{:02} 12:00:00", anno, mese, giorno),
                    "%Y-%m-%d %H:%M:%S"
                ) {
                    return Some(DateTime::from_naive_utc_and_offset(dt, Utc));
                }
            }
        }
        _ => {}
    }
    
    None
}

// Mantenuto per compatibilità
pub fn calcola_proposta(foto: &FotoData) -> Option<DateTime<Utc>> {
    calcola_proposta_con_strategia(foto, &foto.strategia_datetime_original)
}

pub fn rileva_incongruenze(foto: &FotoData) -> Vec<String> {
    use chrono::Datelike;
    let mut incongruenze = Vec::new();
    
    // Confronta EXIF DateTimeOriginal con anno nel filename
    if let Some(exif_dt) = foto.exif_datetime_original {
        let exif_anno = exif_dt.year();
        
        // Confronta con anno nel nome file
        if let Some(anno_nome) = foto.anno_nome {
            if exif_anno != anno_nome {
                incongruenze.push(format!("EXIF anno {} ≠ filename anno {}", exif_anno, anno_nome));
            }
        }
        
        // Confronta con data nel JSON (photoTakenTime) - solo questo campo per il confronto
        // Confronta solo la data (anno/mese/giorno), non l'ora, perché l'ora può differire
        if let Some(json_dt) = foto.data_json {
            let json_anno = json_dt.year();
            let json_mese = json_dt.month();
            let json_giorno = json_dt.day();
            let exif_mese = exif_dt.month();
            let exif_giorno = exif_dt.day();
            
            // Considera un'incongruenza solo se la differenza è di almeno 1 giorno
            // (non considerare differenze di ore/minuti come incongruenze)
            let exif_date = exif_dt.date_naive();
            let json_date = json_dt.date_naive();
            let diff_giorni = (exif_date - json_date).num_days().abs();
            
            if diff_giorni >= 1 {
                incongruenze.push(format!("EXIF {} ≠ JSON photoTakenTime {} (differenza: {} giorni)", 
                    format!("{:04}-{:02}-{:02}", exif_anno, exif_mese, exif_giorno),
                    format!("{:04}-{:02}-{:02}", json_anno, json_mese, json_giorno),
                    diff_giorni));
            }
        }
    } else {
        // EXIF mancante ma abbiamo dati da filename o JSON (solo photoTakenTime)
        if foto.anno_nome.is_some() || foto.data_json.is_some() {
            incongruenze.push("EXIF DateTimeOriginal mancante".to_string());
        }
    }
    
    incongruenze
}

pub fn calcola_gravita_incongruenza(foto: &FotoData) -> i64 {
    use chrono::NaiveDate;
    
    let mut max_diff_giorni = 0i64;
    
    // Confronta EXIF con data nel filename
    if let Some(exif_dt) = foto.exif_datetime_original {
        if let Some((anno_nome, mese_nome, giorno_nome)) = foto.data_nome {
            // Crea una data dal filename
            if let Some(data_nome) = NaiveDate::from_ymd_opt(anno_nome as i32, mese_nome, giorno_nome) {
                let exif_date = exif_dt.date_naive();
                let diff = (exif_date - data_nome).num_days().abs();
                max_diff_giorni = max_diff_giorni.max(diff);
            }
        }
        
        // Confronta EXIF con data nel JSON photoTakenTime (solo questo campo per il confronto)
        // Usa solo la differenza in giorni (non considera differenze di ore/minuti)
        if let Some(json_dt) = foto.data_json {
            let exif_date = exif_dt.date_naive();
            let json_date = json_dt.date_naive();
            let diff = (exif_date - json_date).num_days().abs();
            max_diff_giorni = max_diff_giorni.max(diff);
        }
    } else {
        // EXIF mancante: non calcolare una differenza di giorni perché non c'è nulla da confrontare
        // La gravità rimane 0 - l'incongruenza è solo "EXIF mancante", non una differenza temporale
        // Se ci sono altre incongruenze (es. differenze tra JSON e filename), verranno calcolate sopra
        // ma se l'EXIF è mancante, non possiamo calcolare una differenza con l'EXIF stesso
        max_diff_giorni = 0;
    }
    
    max_diff_giorni
}

pub fn leggi_foto_singola(foto_path: PathBuf) -> FotoData {
    let nome_file = foto_path.file_name().unwrap().to_string_lossy().to_string();
    
    let data_nome = estrai_anno_da_nome(&nome_file);
    let anno_nome = data_nome.map(|(a, _, _)| a);
    
    let json_path = trova_file_json(&foto_path);
    let (data_json, data_json_creation) = json_path.as_ref()
        .map(|p| leggi_data_json_completo(p))
        .unwrap_or((None, None));
    
    let (exif_dt, exif_cd, exif_md) = ottieni_tutti_campi_exif(&foto_path);
    
    let mut foto = FotoData {
        path: foto_path,
        nome_file,
        anno_nome,
        data_nome,
        data_json,
        data_json_creation,
        exif_datetime_original: exif_dt,
        exif_create_date: exif_cd,
        exif_modify_date: exif_md,
        proposta_datetime_original: None,
        proposta_create_date: None,
        proposta_modify_date: None,
        strategia_datetime_original: "nome_file_preferito".to_string(),
        strategia_create_date: "nome_file_preferito".to_string(),
        strategia_modify_date: "nome_file_preferito".to_string(),
        incongruenze: Vec::new(),
        gravita_incongruenza: 0,
    };
    
    // Calcola proposte iniziali usando le strategie di default
    foto.proposta_datetime_original = calcola_proposta_con_strategia(&foto, &foto.strategia_datetime_original);
    foto.proposta_create_date = calcola_proposta_con_strategia(&foto, &foto.strategia_create_date);
    foto.proposta_modify_date = calcola_proposta_con_strategia(&foto, &foto.strategia_modify_date);
    
    // Rileva incongruenze e calcola gravità
    foto.incongruenze = rileva_incongruenze(&foto);
    foto.gravita_incongruenza = calcola_gravita_incongruenza(&foto);
    
    foto
}

pub fn leggi_foto_da_directory(directory: &Path) -> Vec<FotoData> {
    let estensioni = vec!["jpg", "JPG", "jpeg", "JPEG", "orf", "ORF", "nef", "NEF"];
    let mut foto_files = Vec::new();
    
    // Cerca ricorsivamente in tutte le sottocartelle
    for entry in walkdir::WalkDir::new(directory) {
        if let Ok(entry) = entry {
            if entry.file_type().is_file() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if let Some(ext_str) = ext.to_str() {
                        if estensioni.contains(&ext_str) {
                            foto_files.push(path.to_path_buf());
                        }
                    }
                }
            }
        }
    }
    
    // Usa rayon per parallelizzare la lettura
    let foto_list: Vec<FotoData> = foto_files
        .into_par_iter()
        .map(leggi_foto_singola)
        .collect();
    
    let mut foto_list_sorted = foto_list;
    foto_list_sorted.sort_by(|a, b| a.nome_file.cmp(&b.nome_file));
    foto_list_sorted
}

pub fn scrivi_exif_datetime(foto_path: &Path, data: DateTime<Utc>, solo_datetime_original: bool) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;
    
    // Includi anche i secondi nella data
    let data_str = format!("{:04}:{:02}:{:02} {:02}:{:02}:{:02}", 
                          data.year(), data.month(), data.day(), 
                          data.hour(), data.minute(), data.second());
    
    let mut cmd = Command::new("exiftool");
    cmd.arg("-overwrite_original");
    cmd.arg("-q"); // Quiet mode
    
    if solo_datetime_original {
        cmd.arg(format!("-DateTimeOriginal={}", data_str));
    } else {
        cmd.arg(format!("-DateTimeOriginal={}", data_str));
        cmd.arg(format!("-CreateDate={}", data_str));
    }
    
    cmd.arg(foto_path);
    let output = cmd.output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!("exiftool fallito per {}: {} {}", foto_path.display(), stderr, stdout).into());
    }
    
    Ok(())
}

pub fn scrivi_tutti_campi_exif(foto_path: &Path, campi: &[(&str, DateTime<Utc>)]) -> Result<(), String> {
    use std::process::Command;
    
    if campi.is_empty() {
        return Ok(());
    }
    
    // Verifica che exiftool sia disponibile
    let exiftool_check = Command::new("exiftool").arg("-ver").output();
    if exiftool_check.is_err() {
        return Err("exiftool non trovato. Assicurati che sia installato e nel PATH.".to_string());
    }
    
    let mut cmd = Command::new("exiftool");
    cmd.arg("-overwrite_original");
    // Non usare -q per vedere gli errori quando necessario
    cmd.arg("-P"); // Preserve file modification date/time
    
    // Usa -all= per permettere la scrittura anche su file EXIF corrotti
    // Questo è necessario per alcuni file JPG e per tutti i file RAW
    cmd.arg("-all=");
    
    for (nome_campo, data) in campi {
        // Includi anche i secondi nella data
        let data_str = format!("{:04}:{:02}:{:02} {:02}:{:02}:{:02}", 
                              data.year(), data.month(), data.day(), 
                              data.hour(), data.minute(), data.second());
        cmd.arg(format!("-{}={}", nome_campo, data_str));
    }
    
    cmd.arg(foto_path);
    let output = cmd.output();
    
    match output {
        Ok(output_result) => {
            if output_result.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output_result.stderr);
                let stdout = String::from_utf8_lossy(&output_result.stdout);
                let exit_code = output_result.status.code().unwrap_or(-1);
                Err(format!("exiftool fallito per {} (exit code {}):\nSTDOUT: {}\nSTDERR: {}", 
                          foto_path.display(), exit_code, stdout, stderr))
            }
        }
        Err(e) => Err(format!("Errore esecuzione exiftool per {}: {}", 
                             foto_path.display(), e)),
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    // Se viene passato un argomento, usa la CLI
    if args.len() >= 2 {
        let directory = Path::new(&args[1]);
        if !directory.exists() {
            eprintln!("Errore: directory non trovata: {}", directory.display());
            return Ok(());
        }
        
        println!("Correttore Date EXIF - Versione Rust (VELOCISSIMA!)");
        println!("===================================================");
        println!("Lettura foto da: {}", directory.display());
        let start = std::time::Instant::now();
        
        let foto_list = leggi_foto_da_directory(directory);
        
        let elapsed = start.elapsed();
        println!("✅ Trovate {} foto in {:?} (VELOCISSIMO!)", foto_list.len(), elapsed);
        
        let con_exif = foto_list.iter().filter(|f| f.exif_datetime_original.is_some()).count();
        let con_proposte = foto_list.iter().filter(|f| f.proposta_datetime_original.is_some()).count();
        
        println!("\n📊 Statistiche:");
        println!("  Totale foto: {}", foto_list.len());
        println!("  Con EXIF DateTimeOriginal: {}", con_exif);
        println!("  Con proposte di modifica: {}", con_proposte);
        
        if con_proposte > 0 {
            println!("\n📋 Prime 10 foto con proposte:");
            for foto in foto_list.iter().filter(|f| f.proposta_datetime_original.is_some()).take(10) {
                println!("\n  {}", foto.nome_file);
                println!("    Strategia default: {}", foto.strategia_datetime_original);
                
                if let Some(dt_json) = foto.data_json {
                    println!("    data_json dal JSON: {} (ora={}:{}:{})", 
                             dt_json.format("%Y-%m-%d %H:%M:%S"), 
                             dt_json.hour(), dt_json.minute(), dt_json.second());
                } else {
                    println!("    data_json: None");
                }
                
                if let Some(dt) = foto.exif_datetime_original {
                    println!("    EXIF attuale: {} (ora={}:{}:{})", 
                             dt.format("%Y-%m-%d %H:%M:%S"),
                             dt.hour(), dt.minute(), dt.second());
                } else {
                    println!("    EXIF attuale: ❌");
                }
                
                if let Some(dt) = foto.proposta_datetime_original {
                    println!("    Proposta (default): {} (ora={}:{}:{})", 
                             dt.format("%Y-%m-%d %H:%M:%S"),
                             dt.hour(), dt.minute(), dt.second());
                }
                
                // Test con strategia json_photo_taken
                let proposta_json = calcola_proposta_con_strategia(&foto, "json_photo_taken");
                if let Some(dt) = proposta_json {
                    println!("    Test json_photo_taken: {} (ora={}:{}:{})", 
                             dt.format("%Y-%m-%d %H:%M:%S"),
                             dt.hour(), dt.minute(), dt.second());
                } else {
                    println!("    Test json_photo_taken: None");
                }
                
                // Test con strategia nome_file_preferito
                let proposta_nome = calcola_proposta_con_strategia(&foto, "nome_file_preferito");
                if let Some(dt) = proposta_nome {
                    println!("    Test nome_file_preferito: {} (ora={}:{}:{})", 
                             dt.format("%Y-%m-%d %H:%M:%S"),
                             dt.hour(), dt.minute(), dt.second());
                } else {
                    println!("    Test nome_file_preferito: None");
                }
            }
        }
        
        return Ok(());
    }
    
    // Altrimenti avvia la GUI
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 800.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "Correttore Date EXIF Foto",
        options,
        Box::new(|cc| Box::new(gui::CorrectorApp::new(cc))),
    )
}
