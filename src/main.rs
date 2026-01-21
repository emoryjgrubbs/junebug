use std::io::{self, Write};

struct Item {
    indent: bool,
    text: String,
}

fn main() -> io::Result<()> {
    let mut basic_list: Vec<Item> = vec![];
    loop {
        let mut command = String::new();
        print!("ENTER COMMAND ? ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut command)?;

        match command.trim().to_lowercase().as_str() {
            "add" => {
                // get a position in the list to insert
                let mut position = String::new();
                print!("ENTER ITEM POSITION ? ");
                io::stdout().flush()?;
                io::stdin().read_line(&mut position)?;

                if let Ok(verified_pos) = position.trim().parse() && verified_pos <= basic_list.len() {
                    // get text to insert
                    let mut text = String::new();
                    print!("ENTER ITEM TEXT ? ");
                    io::stdout().flush()?;
                    io::stdin().read_line(&mut text)?;

                    basic_list.insert(verified_pos, Item{ indent: false, text: text.trim().to_string().clone() });
                }
                else { println!("INVALID POSITION"); }
            },
            "edit" => {
                // get position to indent
                let mut position = String::new();
                print!("ENTER ITEM POSITION ? ");
                io::stdout().flush()?;
                io::stdin().read_line(&mut position)?;

                if let Ok(verified_pos) = position.trim().parse::<usize>() && verified_pos < basic_list.len() {
                    // get text to insert
                    let mut new_text = String::new();
                    print!("ENTER NEW ITEM TEXT ? ");
                    io::stdout().flush()?;
                    io::stdin().read_line(&mut new_text)?;

                    basic_list[verified_pos].text = new_text.trim().to_string().clone();
                }
                else { println!("INVALID POSITION"); }
            },
            "delete" => {
                // get position to delete
                let mut position = String::new();
                print!("ENTER ITEM POSITION ? ");
                io::stdout().flush()?;
                io::stdin().read_line(&mut position)?;

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
                io::stdout().flush()?;
                io::stdin().read_line(&mut position)?;

                if let Ok(verified_pos) = position.trim().parse::<usize>() && verified_pos > 0 && verified_pos < basic_list.len() {
                    basic_list[verified_pos].indent = true;
                }
                else { println!("INVALID POSITION"); }
            },
            "deindent" => {
                // get position to de-indent
                let mut position = String::new();
                print!("ENTER ITEM POSITION ? ");
                io::stdout().flush()?;
                io::stdin().read_line(&mut position)?;

                if let Ok(verified_pos) = position.trim().parse::<usize>() && verified_pos > 0 && verified_pos < basic_list.len() {
                    basic_list[verified_pos].indent = false;
                }
                else { println!("INVALID POSITION"); }
            },
            "print" => { println!(""); for item in &basic_list { if item.indent { print!("\t"); } println!("- {}", item.text); } println!(""); },
            "exit" => { println!("EXITING.."); break; },
            _ => println!("UNKNOWN COMMAND"),
        }
    }
    Ok(())
}
