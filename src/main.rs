use std::io::{self, Write};
use rusqlite::{params, Connection, Result};

struct Item {
    indent: bool,
    text: String,
}

fn main() -> Result<()>{
    let db = Connection::open_in_memory()?;

    db.execute_batch(
        "
            create table if not exists List(
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              name TEXT NOT NULL,
              check_boxes BOOLEAN NOT NULL DEFAULT(FALSE)
            );
            create table if not exists Item(
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              complete BOOLEAN NOT NULL DEFAULT(FALSE),
              text TEXT NOT NULL,
              list_id INTEGER NOT NULL,
              FOREIGN KEY(list_id) REFERENCES List(list_id)
            );
            create table if not exists Recurrence(
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              period INTEGER NOT NULL,
              time_last DATETIME NOT NULL
            );
        ",
    )?;

    let mut basic_list: Vec<Item> = vec![];
    loop {
        let mut command = String::new();
        print!("ENTER COMMAND ? ");
        io::stdout().flush().expect("'Enter Command' print buffer failed flush");
        io::stdin().read_line(&mut command).expect("Command failed be read");

        match command.trim().to_lowercase().as_str() {
            "add" => {
                // get a position in the list to insert
                let mut position = String::new();
                print!("ENTER ITEM POSITION ? ");
                io::stdout().flush().expect("'Enter Position' print buffer failed flush");
                io::stdin().read_line(&mut position).expect("Position failed to read");

                if let Ok(verified_pos) = position.trim().parse() && verified_pos <= basic_list.len() {
                    // get text to insert
                    let mut text = String::new();
                    print!("ENTER ITEM TEXT ? ");
                    io::stdout().flush().expect("'Enter Text' print buffer failed to flush");
                    io::stdin().read_line(&mut text).expect("Text failed to read");

                    basic_list.insert(verified_pos, Item{ indent: false, text: text.trim().to_string().clone() });
                }
                else { println!("INVALID POSITION"); }
            },
            "edit" => {
                // get position to indent
                let mut position = String::new();
                print!("ENTER ITEM POSITION ? ");
                io::stdout().flush().expect("'Enter Position' print buffer failed flush");
                io::stdin().read_line(&mut position).expect("Position failed to read");

                if let Ok(verified_pos) = position.trim().parse::<usize>() && verified_pos < basic_list.len() {
                    // get text to insert
                    let mut new_text = String::new();
                    print!("ENTER NEW ITEM TEXT ? ");
                    io::stdout().flush().expect("'Enter New Text' print buffer failed to flush");
                    io::stdin().read_line(&mut new_text).expect("New Text failed to read");

                    basic_list[verified_pos].text = new_text.trim().to_string().clone();
                }
                else { println!("INVALID POSITION"); }
            },
            "delete" => {
                // get position to delete
                let mut position = String::new();
                print!("ENTER ITEM POSITION ? ");
                io::stdout().flush().expect("'Enter Position' print buffer failed flush");
                io::stdin().read_line(&mut position).expect("Position failed to read");

                if let Ok(verified_pos) = position.trim().parse() && verified_pos < basic_list.len() {
                    if verified_pos == 0 && basic_list.len() > 1 { basic_list[1].indent = false; }
                    basic_list.remove(verified_pos);
                }
                else { println!("INVALID POSITION"); }
            },
            "indent" => {
                // get position to indent
                let mut position = String::new();
                print!("ENTER ITEM POSITION ? ");
                io::stdout().flush().expect("'Enter Position' print buffer failed flush");
                io::stdin().read_line(&mut position).expect("Position failed to read");

                if let Ok(verified_pos) = position.trim().parse::<usize>() && verified_pos > 0 && verified_pos < basic_list.len() {
                    basic_list[verified_pos].indent = true;
                }
                else { println!("INVALID POSITION"); }
            },
            "deindent" => {
                // get position to de-indent
                let mut position = String::new();
                print!("ENTER ITEM POSITION ? ");
                io::stdout().flush().expect("'Enter Position' print buffer failed flush");
                io::stdin().read_line(&mut position).expect("Position failed to read");

                if let Ok(verified_pos) = position.trim().parse::<usize>() && verified_pos > 0 && verified_pos < basic_list.len() {
                    basic_list[verified_pos].indent = false;
                }
                else { println!("INVALID POSITION"); }
            },
            "check recurrence" => {},
            "print" => { println!(""); for item in &basic_list { if item.indent { print!("\t"); } println!("- {}", item.text); } println!(""); },
            "exit" => { println!("EXITING.."); break; },
            _ => println!("UNKNOWN COMMAND"),
        }
    }
    Ok(())
}
