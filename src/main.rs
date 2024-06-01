use std::cmp;
use std::fs::File;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::thread;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use std::collections::HashMap;
use std::env;
use chrono::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

/*
Erstes Argument: Teilaufgabe A oder B (default: A)
Zweites Argument: Dauer der Runde in Sekunden (default: 5 Sekunden)
Drittes Argument: Anzahl der Spieler (default: 3)
Viertes Argument: IP and Port (default: 127.0.0.1:7878)
*/

const TEILAUFGABE: char = 'A';
const DAUER_DER_RUNDE: u64 = 10; // in Sekunden
const ANZAHL_SPIELER: u64 = 3;
const IP_AND_PORT : &str = "127.0.0.1:7878";

struct Timestamp {
    hours : u32,
    minutes : u32,
    seconds : u32,
    microseconds : u32
}
enum Wurf {
    TypeA(WurfA),
    TypeB(WurfB)
}
trait Printable {
    fn to_string(&self) -> String;
}
struct WurfA {
    augen : u32,
    start : Timestamp,
    end : Timestamp
}
impl Printable for WurfA {
    fn to_string(&self) -> String {
        format!("{} gewürfelt! (START-Nachricht erhalten: {}:{}:{}.{}, Antwort gesendet: {}:{}:{}.{})",
                self.augen,
                self.start.hours, self.start.minutes, self.start.seconds, self.start.microseconds,
                self.end.hours, self.end.minutes, self.end.seconds, self.end.microseconds)
    }
}
struct WurfB {
    augen : u32,
    start : Timestamp,
    end : Timestamp,
    lc : u32
}
impl Printable for WurfB {
    fn to_string(&self) -> String {
        format!("{} gewürfelt! (START-Nachricht erhalten: {}:{}:{}.{}, Antwort gesendet: {}:{}:{}.{}, Lamport-Zeit: {})",
                self.augen,
                self.start.hours, self.start.minutes, self.start.seconds, self.start.microseconds,
                self.end.hours, self.end.minutes, self.end.seconds, self.end.microseconds,
                self.lc
            )
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let teilaufgabe: char = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(TEILAUFGABE);
    let dauer_der_runde: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(DAUER_DER_RUNDE);
    let anzahl_spieler: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(ANZAHL_SPIELER);
    let ip_and_port: &str = args.get(4).map(|s| s.as_str()).unwrap_or(IP_AND_PORT);

    let lc = Arc::new(AtomicU32::new(0));
    let results = Arc::new(Mutex::new(HashMap::new()));

    let mut clients = start_clients(teilaufgabe, &results, anzahl_spieler, &lc, ip_and_port);

    // Create or open a file named "output.txt"
    let mut output_file = File::create("output.txt").unwrap();
    let mut round = 1;
    loop {
        let start_timestamp: DateTime<Local> = Local::now();

        lc.fetch_add(1, Ordering::SeqCst);
        let lc_start = lc.load(Ordering::SeqCst);
        // Send START to all players
        for client in &mut clients {
            //println!("Sende START an Spieler!");
            if teilaufgabe == 'A' {
                let _ = client.write(b"START");
            } else {
                let _ = client.write(format!("START {}", lc_start).as_bytes());
            }
        }

        thread::sleep(Duration::from_secs(dauer_der_runde));
    
        lc.fetch_add(1, Ordering::SeqCst);
        let lc_stop = lc.load(Ordering::SeqCst);
        // Send STOP to all players
        for client in &mut clients {
            if teilaufgabe == 'A' {
                let _ = client.write(b"STOP");
            } else {
                let _ = client.write(format!("STOP {}", lc_stop).as_bytes());
            }
        }

        {
            let mut results_locked = results.lock().unwrap();
            evaluate_results(&mut results_locked, &mut output_file, round, start_timestamp, lc_start, lc_stop, teilaufgabe);
        }
        results.lock().unwrap().clear();
        round += 1;
    }
}

fn start_clients(teilaufgabe : char, results: &Arc<Mutex<HashMap<String, Wurf>>>, anzahl_spieler : u64, lc : &Arc<AtomicU32>, ip_and_port : &str) -> Vec<TcpStream> {
    let listener = TcpListener::bind(ip_and_port).unwrap();
    let mut clients = Vec::new();
    println!("Spielleiter wartet auf {anzahl_spieler} Verbindungen...");
    let mut num_players = 0;

    // Accepting connections
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let mut copy = stream.try_clone().unwrap();
        copy.write(format!("WELCOME {}", teilaufgabe).as_bytes()).expect("WELCOME-Nachricht nicht erfolgreich versendet...");
        clients.push(copy);
        let results = Arc::clone(&results);
        let client_lc = Arc::clone(lc);
        thread::spawn(move || {
            println!("Neuer Spieler verbunden!");
            if teilaufgabe == 'A' {
                handle_client_a(stream, results);
            } else {
                handle_client_b(stream, results, client_lc);
            }
        });

        num_players += 1;
        if num_players == anzahl_spieler {
            break;
        }
    };
    clients
}

fn evaluate_results(
    results : &mut MutexGuard<HashMap<String, Wurf>>,
    output_file : &mut File,
    round : i32,
    start_timestamp : DateTime<Local>,
    lc_start : u32,
    lc_stop : u32,
    teilaufgabe : char
) {
    // remove results with incorrect lamport time
    results.retain(|_, wurf| match wurf {
        Wurf::TypeA(_) => {true} // retain all of type a
        Wurf::TypeB(wurf) => wurf.lc > lc_start && wurf.lc < lc_stop
    });

    let end_round: DateTime<Local> = Local::now();
    writeln!(output_file, "Runde {} - gestartet um {} - beendet um {}:", round, start_timestamp, end_round).expect("Schreiben nicht erfolgreich.");
    if teilaufgabe == 'B' {
        writeln!(output_file, "Lamportzeit zu Rundenstart: {} und zu Rundenende: {}", lc_start, lc_stop).expect("Schreiben nicht erfolgreich.");
    }
    for (player, result) in results.iter() {
        match result {
            Wurf::TypeA(a) => {
                println!("{player} hat eine {}", a.to_string());
                writeln!(output_file, "{player} hat eine {}", a.to_string())
                    .expect("Schreiben nicht erfolgreich.");
            }
            Wurf::TypeB(b) => {
                println!("{player} hat eine {}", b.to_string());
                writeln!(output_file, "{player} hat eine {}", b.to_string())
                    .expect("Schreiben nicht erfolgreich.");
            }
        }
    }
    {
        let win = results.iter().max_by_key(|(_, wurf)| match wurf {
            Wurf::TypeA(a) => {a.augen}
            Wurf::TypeB(b) => b.augen
        });
        if win.is_some(){
            let (winner, wurf) = win.unwrap();
            match wurf {
                Wurf::TypeA(wurf) => {
                    println!("Gewonnen hat {} mit folgendem Wurf: {}!\n", winner, wurf.augen);
                    writeln!(output_file, "Runde {} hat {} mit folgendem Wurf gewonnen: {}!\n", round, winner, wurf.augen)
                        .expect("Schreiben nicht erfolgreich.");
                }
                Wurf::TypeB(wurf) => {
                    println!("Gewonnen hat {} mit folgendem Wurf: {}!\n", winner, wurf.augen);
                    writeln!(output_file, "Runde {} hat {} mit folgendem Wurf gewonnen: {}!\n", round, winner, wurf.augen)
                        .expect("Schreiben nicht erfolgreich.");
                }
            }
        } else {
            println!("Da waren wohl alle Spieler zu langsam für den Spielleiter :)\n");
            writeln!(output_file, "Da waren wohl alle Spieler zu langsam für den Spielleiter :)\n").expect("Schreiben nicht erfolgreich.");
        }
    }
}

fn handle_client_a(stream: TcpStream, results: Arc<Mutex<HashMap<String, Wurf>>>) {
    handle_client_general(stream, |message : &str| {
        if message.starts_with("WURF") {
            let parts: Vec<&str> = message.trim().split_whitespace().collect();
            if parts.len() == 11 {
                let name = parts[1].to_string();
                let wurf = extract_wurf_a(parts);
                let mut results = results.lock().unwrap();
                //println!("Resultat hinzugefügt für {}: {}", name, wurf.augen);
                results.insert(name, Wurf::TypeA(wurf));
            } else {
                println!("Wrong message format!");
            }
        } else if message.trim().is_empty() {
            panic!("READ EMPTY MESSAGE, PANICKING!");
        }
    });
}

fn handle_client_b(stream: TcpStream, results: Arc<Mutex<HashMap<String, Wurf>>>, lc : Arc<AtomicU32>) {
    handle_client_general(stream, |message : &str| {
        if message.starts_with("WURF") {
            let parts: Vec<&str> = message.trim().split_whitespace().collect();
            if parts.len() == 12 {
                let name = parts[1].to_string();
                let wurf = extract_wurf_b(parts);

                // max(lc, client_lc)
                lc.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current|
                    Some(cmp::max(wurf.lc, current))
                ).expect("Lamport-Zeit konnte nicht geupdated werden.");
                // lc++
                lc.fetch_add(1, Ordering::SeqCst);

                let mut results = results.lock().unwrap();
                //println!("Resultat hinzugefügt für {}: {}", name, wurf.augen);
                results.insert(name, Wurf::TypeB(wurf));
            } else {
                println!("Wrong message format!");
            }
        } else if message.trim().is_empty() {
            panic!("READ EMPTY MESSAGE, PANICKING!");
        }
    });
}

fn handle_client_general<F>(mut stream: TcpStream, handle_message: F)
where F : Fn(&str)
{
    let mut buffer = [0; 512];
    while match stream.read(&mut buffer) {
        Ok(size) => {
            let message = String::from_utf8_lossy(&buffer[..size]);
            println!("Nachricht erhalten: {message}");
            handle_message(&message);
            true
        },
        Err(_) => {
            false
        }
    } {}
}

fn extract_wurf_a(parts: Vec<&str>) -> WurfA {
    let augen: u32 = parts[2].parse().unwrap_or(0);
    let start_hours: u32 = parts[3].parse().unwrap_or(0);
    let start_minutes: u32 = parts[4].parse().unwrap_or(0);
    let start_seconds: u32 = parts[5].parse().unwrap_or(0);
    let start_micros: u32 = parts[6].parse().unwrap_or(0);
    let end_hours: u32 = parts[7].parse().unwrap_or(0);
    let end_minutes: u32 = parts[8].parse().unwrap_or(0);
    let end_seconds: u32 = parts[9].parse().unwrap_or(0);
    let end_micros: u32 = parts[10].parse().unwrap_or(0);
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
    let wurf = WurfA {
        augen,
        start,
        end
    };
    wurf
}

fn extract_wurf_b(parts: Vec<&str>) -> WurfB {
    let augen: u32 = parts[2].parse().unwrap_or(0);
    let lc : u32 = parts[3].parse().expect("Lamport-Zeit wird unbedingt gebraucht!");
    let start_hours: u32 = parts[4].parse().unwrap_or(0);
    let start_minutes: u32 = parts[5].parse().unwrap_or(0);
    let start_seconds: u32 = parts[6].parse().unwrap_or(0);
    let start_micros: u32 = parts[7].parse().unwrap_or(0);
    let end_hours: u32 = parts[8].parse().unwrap_or(0);
    let end_minutes: u32 = parts[9].parse().unwrap_or(0);
    let end_seconds: u32 = parts[10].parse().unwrap_or(0);
    let end_micros: u32 = parts[11].parse().unwrap_or(0);
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
    let wurf = WurfB {
        augen,
        start,
        end,
        lc
    };
    wurf
}
