use exif::{In, Tag, Value};
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc, NaiveDateTime, Datelike, Timelike};
use rayon::prelude::*;

#[derive(Debug, Clone)]
struct FotoData {
    #[allow(dead_code)]
    path: PathBuf,
    nome_file: String,
    #[allow(dead_code)]
    anno_nome: Option<i32>,
    data_nome: Option<(i32, u32, u32)>, // (anno, mese, giorno)
    data_json: Option<DateTime<Utc>>,
    exif_datetime_original: Option<DateTime<Utc>>,
    #[allow(dead_code)]
    exif_create_date: Option<DateTime<Utc>>,
    #[allow(dead_code)]
    exif_modify_date: Option<DateTime<Utc>>,
    proposta_datetime_original: Option<DateTime<Utc>>,
    strategia: String,
}

#[derive(Debug, Deserialize)]
struct GooglePhotoJson {
    #[serde(rename = "photoTakenTime")]
    photo_taken_time: Option<PhotoTime>,
}

#[derive(Debug, Deserialize)]
struct PhotoTime {
    timestamp: String,
}

fn estrai_anno_da_nome(nome_file: &str) -> Option<(i32, u32, u32)> {
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

fn trova_file_json(foto_path: &Path) -> Option<PathBuf> {
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

fn leggi_data_json(json_path: &Path) -> Option<DateTime<Utc>> {
    let content = fs::read_to_string(json_path).ok()?;
    let json: GooglePhotoJson = serde_json::from_str(&content).ok()?;
    
    if let Some(photo_time) = json.photo_taken_time {
        if let Ok(timestamp) = photo_time.timestamp.parse::<i64>() {
            return DateTime::from_timestamp(timestamp, 0);
        }
    }
    
    None
}

fn leggi_exif_datetime(file_path: &Path, tag: Tag) -> Option<DateTime<Utc>> {
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

fn ottieni_tutti_campi_exif(foto_path: &Path) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
    let datetime_original = leggi_exif_datetime(foto_path, Tag::DateTimeOriginal);
    let create_date = leggi_exif_datetime(foto_path, Tag::DateTimeDigitized);
    let modify_date = leggi_exif_datetime(foto_path, Tag::DateTime);
    
    (datetime_original, create_date, modify_date)
}

fn calcola_proposta(foto: &FotoData) -> Option<DateTime<Utc>> {
    match foto.strategia.as_str() {
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
        "json" => {
            return foto.data_json;
        }
        "exif_attuale" => {
            return foto.exif_datetime_original;
        }
        "nome_file_preferito" => {
            if let Some((anno, mese, giorno)) = foto.data_nome {
                if let Ok(dt) = NaiveDateTime::parse_from_str(
                    &format!("{:04}-{:02}-{:02} 12:00:00", anno, mese, giorno),
                    "%Y-%m-%d %H:%M:%S"
                ) {
                    return Some(DateTime::from_naive_utc_and_offset(dt, Utc));
                }
            }
            return foto.data_json;
        }
        "json_preferito" => {
            if let Some(dt) = foto.data_json {
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

fn leggi_foto_singola(foto_path: PathBuf) -> FotoData {
    let nome_file = foto_path.file_name().unwrap().to_string_lossy().to_string();
    
    let data_nome = estrai_anno_da_nome(&nome_file);
    let anno_nome = data_nome.map(|(a, _, _)| a);
    
    let json_path = trova_file_json(&foto_path);
    let data_json = json_path.as_ref().and_then(|p| leggi_data_json(p));
    
    let (exif_dt, exif_cd, exif_md) = ottieni_tutti_campi_exif(&foto_path);
    
    let mut foto = FotoData {
        path: foto_path,
        nome_file,
        anno_nome,
        data_nome,
        data_json,
        exif_datetime_original: exif_dt,
        exif_create_date: exif_cd,
        exif_modify_date: exif_md,
        proposta_datetime_original: None,
        strategia: "nome_file_preferito".to_string(),
    };
    
    foto.proposta_datetime_original = calcola_proposta(&foto);
    
    foto
}

fn leggi_foto_da_directory(directory: &Path) -> Vec<FotoData> {
    let estensioni = vec!["jpg", "JPG", "jpeg", "JPEG"];
    let mut foto_files = Vec::new();
    
    for entry in walkdir::WalkDir::new(directory).max_depth(1) {
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

#[allow(dead_code)]
fn scrivi_exif_datetime(foto_path: &Path, data: DateTime<Utc>, solo_datetime_original: bool) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;
    
    let data_str = format!("{:04}:{:02}:{:02} {:02}:{:02}:00", 
                          data.year(), data.month(), data.day(), 
                          data.hour(), data.minute());
    
    let mut cmd = Command::new("exiftool");
    cmd.arg("-overwrite_original");
    
    if solo_datetime_original {
        cmd.arg(format!("-DateTimeOriginal={}", data_str));
    } else {
        cmd.arg(format!("-DateTimeOriginal={}", data_str));
        cmd.arg(format!("-CreateDate={}", data_str));
        cmd.arg(format!("-ModifyDate={}", data_str));
    }
    
    cmd.arg(foto_path);
    cmd.output()?;
    
    Ok(())
}

fn main() {
    println!("Correttore Date EXIF - Versione Rust (VELOCISSIMA!)");
    println!("===================================================");
    
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Uso: {} <directory>", args[0]);
        return;
    }
    
    let directory = Path::new(&args[1]);
    if !directory.exists() {
        eprintln!("Errore: directory non trovata: {}", directory.display());
        return;
    }
    
    println!("Lettura foto da: {}", directory.display());
    let start = std::time::Instant::now();
    
    let foto_list = leggi_foto_da_directory(directory);
    
    let elapsed = start.elapsed();
    println!("✅ Trovate {} foto in {:?} (VELOCISSIMO!)", foto_list.len(), elapsed);
    
    // Mostra statistiche
    let con_exif = foto_list.iter().filter(|f| f.exif_datetime_original.is_some()).count();
    let con_proposte = foto_list.iter().filter(|f| f.proposta_datetime_original.is_some()).count();
    
    println!("\n📊 Statistiche:");
    println!("  Totale foto: {}", foto_list.len());
    println!("  Con EXIF DateTimeOriginal: {}", con_exif);
    println!("  Con proposte di modifica: {}", con_proposte);
    
    // Mostra prime 10 foto con proposte
    if con_proposte > 0 {
        println!("\n📋 Prime 10 foto con proposte:");
        for foto in foto_list.iter().filter(|f| f.proposta_datetime_original.is_some()).take(10) {
            println!("  {}", foto.nome_file);
            if let Some(dt) = foto.exif_datetime_original {
                println!("    EXIF attuale: {}", dt.format("%Y-%m-%d %H:%M:%S"));
            } else {
                println!("    EXIF attuale: ❌");
            }
            if let Some(dt) = foto.proposta_datetime_original {
                println!("    Proposta: {}", dt.format("%Y-%m-%d %H:%M:%S"));
            }
        }
    }
}
