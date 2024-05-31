use std::fs::File;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::thread;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::collections::HashMap;
use std::env;
use chrono::prelude::*;

const DAUER_DER_RUNDE: u64 = 5; // in Sekunden
const ANZAHL_SPIELER: usize = 3;

struct Timestamp {
    hours : u32,
    minutes : u32,
    seconds : u32,
    microseconds : u32
}
struct Wurf {
    augen : u32,
    start : Timestamp,
    end : Timestamp
}

fn handle_client(mut stream: TcpStream, results: Arc<Mutex<HashMap<String, Wurf>>>) {
    let mut buffer = [0; 512];
    while match stream.read(&mut buffer) {
        Ok(size) => {
            let message = String::from_utf8_lossy(&buffer[..size]);
            println!("Nachricht erhalten: {message}");
            if message.starts_with("WURF") {
                let parts: Vec<&str> = message.trim().split_whitespace().collect();
                if parts.len() == 11 {
                    let name = parts[1].to_string();
                    let wurf = extract_wurf(parts);
                    let mut results = results.lock().unwrap();
                    //println!("Resultat hinzugefügt für {}: {}", name, wurf.augen);
                    results.insert(name, wurf);
                } else {
                    println!("Wrong message format!");
                }
            } else if message.trim().is_empty() {
                panic!("READ EMPTY MESSAGE, PANICKING!");
            }
            true
        },
        Err(_) => {
            false
        }
    } {}
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let dauer_der_runde: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(DAUER_DER_RUNDE);

    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    let results = Arc::new(Mutex::new(HashMap::new()));
    let mut clients = Vec::new();

    println!("Spielleiter wartet auf {ANZAHL_SPIELER} Verbindungen...");

    let mut num_players = 0;

    // Accepting connections
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        clients.push(stream.try_clone().unwrap());
        let results = Arc::clone(&results);

        thread::spawn(move || {
            println!("Neuer Spieler verbunden!");
            handle_client(stream, results);
        });

        num_players += 1;
        if num_players == ANZAHL_SPIELER {
            break;
        }
    }

    // Create or open a file named "output.txt"
    let mut file = File::create("output.txt").unwrap();
    let mut round = 1;
    loop {
        let start_round: DateTime<Local> = Local::now();

        // Send START to all players
        for client in &mut clients {
            //println!("Sende START an Spieler!");
            let _ = client.write(b"START\n");
        }

        thread::sleep(Duration::from_secs(dauer_der_runde));
    
        // Send STOP to all players
        for client in &mut clients {
            let _ = client.write(b"STOP\n");
        }

        {
            // Evaluate results
            let results_locked = results.lock().unwrap();
            let end_round: DateTime<Local> = Local::now();
            writeln!(file, "Runde {} - gestartet um {} - beendet um {}:", round, start_round, end_round).expect("Schreiben nicht erfolgreich.");
    
            for (player, result) in results_locked.iter() {
                println!("{player} hat eine {} gewürfelt! (START-Nachricht erhalten: {}:{}:{}-{}, Antwort gesendet: {}:{}:{}-{})",
                        result.augen,
                        result.start.hours, result.start.minutes, result.start.seconds, result.start.microseconds,
                        result.end.hours, result.end.minutes, result.end.seconds, result.end.microseconds);
                writeln!(file, "{player} hat eine {} gewürfelt! (START-Nachricht erhalten: {}:{}:{}-{}, Antwort gesendet: {}:{}:{}-{})",
                        result.augen,
                        result.start.hours, result.start.minutes, result.start.seconds, result.start.microseconds,
                        result.end.hours, result.end.minutes, result.end.seconds, result.end.microseconds)
                    .expect("Schreiben nicht erfolgreich.");
            }
    
            {
                let win = results_locked.iter().max_by_key(|entry| entry.1.augen);
                if win.is_some(){
                    let (winner, wurf) = win.unwrap();
                    println!("Gewonnen hat {} mit folgendem Wurf: {}!\n", winner, wurf.augen);
                    writeln!(file, "Runde {} hat {} mit folgendem Wurf gewonnen: {}!\n", round, winner, wurf.augen).expect("Schreiben nicht erfolgreich.");
                } else {
                    println!("Da waren wohl alle Spieler zu langsam für den Spielleiter :)\n");
                    writeln!(file, "Da waren wohl alle Spieler zu langsam für den Spielleiter :)\n").expect("Schreiben nicht erfolgreich.");
                }
            }
            round += 1;
        }
        results.lock().unwrap().clear();
    }
}

fn extract_wurf(parts: Vec<&str>) -> Wurf {
    let augen: u32 = parts[2].parse().unwrap_or(0);
    let start_hours: u32 = parts[3].parse().unwrap_or(0);
    let start_minutes: u32 = parts[3].parse().unwrap_or(0);
    let start_seconds: u32 = parts[3].parse().unwrap_or(0);
    let start_micros: u32 = parts[4].parse().unwrap_or(0);
    let end_hours: u32 = parts[3].parse().unwrap_or(0);
    let end_minutes: u32 = parts[3].parse().unwrap_or(0);
    let end_seconds: u32 = parts[5].parse().unwrap_or(0);
    let end_micros: u32 = parts[6].parse().unwrap_or(0);
    let start = Timestamp {
        hours : start_hours,
        minutes : start_minutes,
        seconds : start_seconds,
        microseconds : start_micros
    };
    let end = Timestamp {
        hours : end_hours,
        minutes : end_minutes,
        seconds : end_seconds,
        microseconds : end_micros
    };
    let wurf = Wurf {
        augen,
        start,
        end
    };
    wurf
}
