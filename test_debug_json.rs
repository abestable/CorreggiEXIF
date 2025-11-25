// Test per verificare la lettura del JSON
use std::path::Path;
use std::fs;
use chrono::{DateTime, Utc, Timelike};

fn main() {
    let json_path = Path::new("/home/alberto/takeout_photo/Takeout/Google Foto/Miglior Foto_ Nudi/pfoto20050728_3025.jpg.supplemental-metadata.json");
    
    // Simula quello che fa leggi_data_json_completo
    let content = match fs::read_to_string(json_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Errore lettura file: {}", e);
            return;
        }
    };
    
    println!("=== CONTENUTO JSON ===");
    println!("{}", content);
    println!();
    
    // Parse JSON
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Errore parsing JSON: {}", e);
            return;
        }
    };
    
    // Estrai timestamp
    if let Some(photo_taken) = json.get("photoTakenTime") {
        if let Some(timestamp_str) = photo_taken.get("timestamp").and_then(|v| v.as_str()) {
            println!("=== TIMESTAMP DAL JSON ===");
            println!("Timestamp string: {}", timestamp_str);
            
            if let Ok(ts) = timestamp_str.parse::<i64>() {
                println!("Timestamp i64: {}", ts);
                
                // Usa chrono per convertire (come nel codice)
                if let Some(dt) = DateTime::from_timestamp(ts, 0) {
                    println!("DateTime<Utc>: {}", dt.format("%Y-%m-%d %H:%M:%S"));
                    println!("Ora: {}:{}:{}", dt.hour(), dt.minute(), dt.second());
                    println!("Formato completo: {}", dt);
                } else {
                    eprintln!("Errore: DateTime::from_timestamp ha restituito None");
                }
            } else {
                eprintln!("Errore: impossibile parsare timestamp come i64");
            }
        }
    }
}

