// Test per verificare il problema della proposta con ora 12:00:00
use corrigi_exif::{leggi_foto_da_directory, calcola_proposta_con_strategia};
use std::path::PathBuf;
use chrono::Timelike;

fn main() {
    let directory = PathBuf::from("/home/alberto/takeout_photo/Takeout/Google Foto/Miglior Foto_ Nudi");
    
    println!("=== CARICAMENTO FOTO DALLA DIRECTORY ===");
    let foto_list = leggi_foto_da_directory(&directory);
    
    println!("\n=== ANALISI DELLE PROPOSTE ===");
    let mut count = 0;
    for foto in &foto_list {
        // Mostra solo alcune foto per non inondare l'output
        if foto.nome_file.contains("pfoto20050728") || 
           (foto.nome_file.contains("IMG_2023") && count < 3) ||
           (foto.nome_file.contains("2002") && count < 5) {
            count += 1;
            
            println!("\n--- File: {} ---", foto.nome_file);
            println!("  Strategia default: {}", foto.strategia_datetime_original);
            
            if let Some(ref dt_json) = foto.data_json {
                println!("  data_json dal JSON: {} (ora={}:{}:{})", 
                         dt_json.format("%Y-%m-%d %H:%M:%S"), 
                         dt_json.hour(), dt_json.minute(), dt_json.second());
            } else {
                println!("  data_json: None");
            }
            
            if let Some(ref dt_proposta) = foto.proposta_datetime_original {
                println!("  proposta_datetime_original: {} (ora={}:{}:{})", 
                         dt_proposta.format("%Y-%m-%d %H:%M:%S"), 
                         dt_proposta.hour(), dt_proposta.minute(), dt_proposta.second());
            } else {
                println!("  proposta_datetime_original: None");
            }
            
            // Test con strategia json_photo_taken
            let proposta_json = calcola_proposta_con_strategia(&foto, "json_photo_taken");
            if let Some(ref dt) = proposta_json {
                println!("  Test json_photo_taken: {} (ora={}:{}:{})", 
                         dt.format("%Y-%m-%d %H:%M:%S"), 
                         dt.hour(), dt.minute(), dt.second());
            } else {
                println!("  Test json_photo_taken: None");
            }
            
            // Test con strategia nome_file_preferito
            let proposta_nome = calcola_proposta_con_strategia(&foto, "nome_file_preferito");
            if let Some(ref dt) = proposta_nome {
                println!("  Test nome_file_preferito: {} (ora={}:{}:{})", 
                         dt.format("%Y-%m-%d %H:%M:%S"), 
                         dt.hour(), dt.minute(), dt.second());
            } else {
                println!("  Test nome_file_preferito: None");
            }
        }
    }
    
    println!("\n=== TOTALE FOTO CARICATE: {} ===", foto_list.len());
}
