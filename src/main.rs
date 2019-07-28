use failure::Error;
use std::env;
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
struct Item {
  message: String,
  time: Duration,
  start: Instant,
}

impl fmt::Display for Item {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self.time.checked_sub(self.start.elapsed()) {
      Some(left) => {
        let hours = left.as_secs() / 3200;
        let mins = (left.as_secs() % 3200) / 60;
        let secs = (left.as_secs() % 3200) % 60;
        write!(f, "{} {:02}:{:02}:{:02}", self.message, hours, mins, secs)
      }
      _ => write!(f, "{} done", self.message),
    }
  }
}

fn printstuff(rx: mpsc::Receiver<Item>) {
  let mut items = Vec::new();
  let mut last_printed = 0;

  loop {
    if let Ok(item) = rx.try_recv() {
      items.push(item)
    }

    if last_printed > 0 {
      print!("\x1B[{}A", last_printed);
      for _ in 0..last_printed {
        println!("                                     ");
      }
      print!("\x1B[{}A", last_printed);
    }
    for item in &items {
      if item.start.elapsed() > item.time {
        println!("\x07 {} done!", item.message);
        thread::spawn(|| {
          Command::new("sh")
            .arg("-c")
            .arg("paplay --raw ~/alarm.wav")
            .output()
            .unwrap()
        });
      }
      println!("\r{}", item);
    }
    std::io::stdout().flush().unwrap();

    last_printed = items.len();
    items.retain(|item| item.start.elapsed() <= item.time);

    thread::sleep(Duration::from_millis(1000));
  }
}

fn main() -> Result<(), Error> {
  let mut args: Vec<String> = env::args().collect();
  if args.len() > 1 {
    let mut writer = UnixStream::connect("/tmp/balarm.sock")?;
    args.drain(0..1);
    writer.write_all(args.join(" ").as_bytes())?;
    std::process::exit(0);
  }

  std::fs::remove_file("/tmp/balarm.sock")?;

  let (tx, rx) = mpsc::channel();

  thread::spawn(|| printstuff(rx));

  let listener = UnixListener::bind("/tmp/balarm.sock")?;
  for stream in listener.incoming() {
    match stream {
      Ok(stream) => {
        let stream = BufReader::new(stream);
        for line in stream.lines() {
          let line = line?;
          let parts: Vec<&str> = line.split_whitespace().collect();
          tx.send(Item {
            message: parts[0].to_owned(),
            start: Instant::now(),
            time: Duration::new(parts[1].parse::<u64>()? * 60, 0),
          })?;
        }
      }
      Err(err) => {
        println!("Error: {}", err);
        break;
      }
    }
  }

  Ok(())
}
