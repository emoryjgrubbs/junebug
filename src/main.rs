use std::io::{self, Write};
use rusqlite::{Connection, Transaction, Result, params};
use chrono::{DateTime, Local, TimeZone, NaiveDateTime};

#[derive(Debug, Clone)]
struct List {
    id: i32,
    name: String,
    check_boxes: bool,
    archived: bool,
    hide_complete: bool,
    time_created: i64,
    time_edited: i64,
    item_count: i32,
}

#[derive(Debug, Clone)]
struct Item {
    id: i32,
    complete: bool,
    text: String,
    parent: Option<i32>,
    next: Option<i32>,
    recurrence: Option<Recurrence>,
    schedule_date: Option<i64>,
}
#[derive(Debug, Clone)]
struct IndexedItem {
    index: usize,
    item: Item,
}

#[derive(Debug, Clone)]
struct Recurrence {
    period: i64,
    time_last: i64,
}

fn main() -> Result<()>{
    let path = prompt_usr("Enter DB Path? ".to_string()) + ".db";
    let mut db = Connection::open(path)?;
    initialize_db(&db)?;
    let mut current_list: Option<List> = None;
    let mut item_list: Option<Vec<Item>> = None;
    'list: loop {
        let list_command = prompt_usr("Enter List Command? ".to_string());
        match list_command.to_lowercase().as_str() {
            "add" => {
                let list_name = prompt_usr("Enter List Name? ".to_string());
                let check_boxes_input = prompt_usr("Use Completion? [Y/n] ".to_string());
                let check_boxes = {
                    if check_boxes_input.to_lowercase().as_str() == "n" { false }
                    else { true }
                };
                add_list(&db, list_name.clone(), check_boxes)?;
                let inserted_id = db.query_one(
                    "select LAST_INSERT_ROWID()", [],
                    |row| Ok(row.get::<usize, i32>(0)?)
                )?;
                current_list = Some(get_list_info(&db, inserted_id)?);
                item_list = Some(vec![]);
            },
            "edit" => {
                let list_id = prompt_usr("Enter List Id? ".to_string());
                if let Ok(id_int) = list_id.parse::<i32>() {
                    edit_list(&db, id_int)?;
                    current_list = Some(get_list_info(&db, id_int)?);
                    if let Some(ref unwrapped_list) = current_list {
                        item_list = update_items(&db, unwrapped_list)?;
                    }
                }
            },
            "check_boxes" => {
                let list_id = prompt_usr("Enter List Id? ".to_string());
                if let Ok(id_int) = list_id.parse::<i32>() {
                    toggle_list_boxes(&db, id_int)?;
                    if let Some(ref mut unwrapped_list) = current_list && unwrapped_list.id == id_int {
                        unwrapped_list.check_boxes = !unwrapped_list.check_boxes;
                    }
                }
            },
            "archive" => {
                let list_id = prompt_usr("Enter List Id? ".to_string());
                if let Ok(id_int) = list_id.parse::<i32>() {
                    archive_list(&db, id_int)?;
                    if let Some(ref mut unwrapped_list) = current_list && unwrapped_list.id == id_int {
                        unwrapped_list.archived = !unwrapped_list.archived;
                    }
                }
            },
            "delete" => {
                let list_id = prompt_usr("Enter List Id? ".to_string());
                if let Ok(id_int) = list_id.parse::<i32>() {
                    delete_list(&db, id_int)?;
                    if let Some(ref mut unwrapped_list) = current_list && unwrapped_list.id == id_int {
                        current_list = None;
                        item_list = None;
                    }
                }
            },
            "print" => {
                print_lists(&db)?;
            },
            "exit" => { println!("\tExiting.."); break 'list; },
            _ => println!("\tUnknown Command"),
        }
        if let Some(ref mut unwrapped_list) = current_list.clone() {
            'item: loop {
                let item_command = prompt_usr("Enter Item Command? ".to_string());
                match item_command.to_lowercase().as_str() {
                    "add" => {
                        let item_pos = prompt_usr("Enter Item Position? ".to_string());
                        if let Ok(pos_int) = item_pos.parse::<usize>() {
                            let item_list_len = item_list.clone().unwrap().len();
                            if pos_int <= item_list_len {
                                let next = {
                                    if pos_int == item_list_len { None }
                                    else {
                                        Some(item_list.clone().unwrap()[pos_int].id)
                                    } 
                                };
                                let item_text = prompt_usr("Enter Item Text? ".to_string());
                                if let Some(ref mut item_list_unwrap) = item_list {
                                    add_item(&mut db, pos_int, item_text, next, unwrapped_list.id, item_list_unwrap)?;
                                }
                            }
                        }
                        else { println!("\tError While Parsing Position as usize"); }
                    },
                    "edit" => {
                        let item_pos = prompt_usr("Enter Item Position? ".to_string());
                        if let Ok(pos_int) = item_pos.parse::<usize>() {
                            let item_list_len = item_list.clone().unwrap().len();
                            if pos_int < item_list_len {
                                let new_text = prompt_usr("Enter Item Text? ".to_string());
                                if let Some(ref mut item_list_unwrap) = item_list {
                                    edit_item(&mut db, pos_int, new_text, unwrapped_list.id, item_list_unwrap)?;
                                }
                            }
                        }
                        else { println!("\tError While Parsing Position as usize"); }
                    },
                    "delete" => {
                        let item_pos = prompt_usr("Enter Item Position? ".to_string());
                        if let Ok(pos_int) = item_pos.parse::<usize>() {
                            let item_list_len = item_list.clone().unwrap().len();
                            if pos_int < item_list_len {
                                if let Some(ref mut item_list_unwrap) = item_list {
                                    if let Err(_) = delete_item(&mut db, pos_int, unwrapped_list.id, item_list_unwrap) {
                                        item_list = update_items(&db, unwrapped_list)?;
                                    }
                                }
                            }
                        }
                        else { println!("\tError While Parsing Position as usize"); }
                    },
                    "move" => {
                        let item_pos_old = prompt_usr("Enter Item Position? ".to_string());
                        let item_pos_new = prompt_usr("Enter Destination Position? ".to_string());
                        if let Ok(old_int) = item_pos_old.parse::<usize>() && let Ok(new_int) = item_pos_new.parse::<usize>() {
                            let item_list_len = item_list.clone().unwrap().len();
                            if old_int < item_list_len && new_int < item_list_len {
                                if let Some(ref mut item_list_unwrap) = item_list {
                                    if let Err(_) = move_item(&mut db, old_int, new_int, unwrapped_list.id, item_list_unwrap) {
                                        item_list = update_items(&db, unwrapped_list)?;
                                    }
                                }
                            }
                        }
                        else { println!("\tError While Parsing Position as usize"); }
                    },
                    "indent" => {
                        let item_pos = prompt_usr("Enter Item Position? ".to_string());
                        if let Ok(pos_int) = item_pos.parse::<usize>() {
                            let item_list_len = item_list.clone().unwrap().len();
                            if pos_int < item_list_len {
                                if let Some(ref mut item_list_unwrap) = item_list {
                                    indent_item(&mut db, pos_int, unwrapped_list.id, item_list_unwrap)?;
                                }
                            }
                        }
                        else { println!("\tError While Parsing Position as usize"); }
                    },
                    "recur" => {
                        let item_pos = prompt_usr("Enter Item Position? ".to_string());
                        if let Ok(pos_int) = item_pos.parse::<usize>() {
                            let item_list_len = item_list.clone().unwrap().len();
                            if pos_int < item_list_len {
                                let time_start = {
                                    let remove = prompt_usr("Remove Recurrence? [y/N] ".to_string());
                                    if remove == "y" {
                                        "remove".to_string()
                                    }
                                    else {
                                        let start_now = prompt_usr("Start Recurrence Now? [Y/n] ".to_string());
                                        if start_now == "n" {
                                            let year = prompt_usr("Enter Start Year? ".to_string());
                                            let month = prompt_usr("Enter Start Month? ".to_string());
                                            let day = prompt_usr("Enter Start Day? ".to_string());
                                            let hour = prompt_usr("Enter Start Hour? ".to_string());
                                            let minute = prompt_usr("Enter Start Minute? ".to_string());
                                            // create date as yyyy-mm-dd
                                            let date = format!("{:0>4}-", year).to_owned() + format!("{:0>2}-", month).as_str() + format!("{:0>2}", day).as_str();
                                            // create time as hh:mm
                                            let time = format!("{:0>2}:", hour).to_owned() + format!("{:0>2}:00", minute).as_str();
                                            let datetime = date + "T" + time.as_str();
                                            datetime
                                        }
                                        else { "now".to_string() }
                                    }
                                }; 
                                let period = {
                                    if time_start != "remove" {
                                        let mut days = prompt_usr("Enter Days Between Recurrences? ".to_string());
                                        let mut hours = prompt_usr("Enter Hours Between Recurrences? ".to_string());
                                        let mut minutes = prompt_usr("Enter Minutes Between Recurrences? ".to_string());
                                        let mut seconds = prompt_usr("Enter Seconds Between Recurrences? ".to_string());
                                        if days == "" { days = "0".to_string(); }
                                        if hours == "" { hours = "0".to_string(); }
                                        if minutes == "" { minutes = "0".to_string(); }
                                        if seconds == "" { seconds = "0".to_string(); }
                                        if let Ok(days_int) = days.parse::<i64>() &&
                                            let Ok(hours_int) = hours.parse::<i64>() &&
                                            let Ok(minutes_int) = minutes.parse::<i64>() &&
                                            let Ok(seconds_int) = seconds.parse::<i64>()
                                        {
                                            if days_int >= 0 && hours_int >= 0 && minutes_int >= 0 && seconds_int >= 0 {
                                                let mut period_calculation = seconds_int;
                                                period_calculation += 60 * minutes_int;
                                                period_calculation += 3600 * hours_int;
                                                period_calculation += 86400 * days_int;
                                                Some(period_calculation)
                                            }
                                            else {
                                                println!("\tError Invalid Period Value");
                                                None
                                            }
                                        }
                                        else {
                                            println!("\tError While Parsing Period as i32");
                                            None
                                        }
                                    }
                                    else { None }
                                };
                                if let Some(ref mut item_list_unwrap) = item_list && let Some(unwrapped_period) = period {
                                    recur_item(&mut db, pos_int, unwrapped_period, time_start, unwrapped_list.id, item_list_unwrap)?;
                                }
                            }
                        }
                        else { println!("\tError While Parsing Position as usize"); }
                    },
                    "schedule" => {
                        let item_pos = prompt_usr("Enter Item Position? ".to_string());
                        if let Ok(pos_int) = item_pos.parse::<usize>() {
                            let item_list_len = item_list.clone().unwrap().len();
                            if pos_int < item_list_len {
                                let time_start = {
                                    let remove = prompt_usr("Remove Schedule? [y/N] ".to_string());
                                    if remove == "y" {
                                        "remove".to_string()
                                    }
                                    else {
                                        let year = prompt_usr("Enter Schedule Year? ".to_string());
                                        let month = prompt_usr("Enter Schedule Month? ".to_string());
                                        let day = prompt_usr("Enter Schedule Day? ".to_string());
                                        let hour = prompt_usr("Enter Schedule Hour? ".to_string());
                                        let minute = prompt_usr("Enter Schedule Minute? ".to_string());
                                        // create date as yyyy-mm-dd
                                        let date = format!("{:0>4}-", year).to_owned() + format!("{:0>2}-", month).as_str() + format!("{:0>2}", day).as_str();
                                        // create time as hh:mm
                                        let time = format!("{:0>2}:", hour).to_owned() + format!("{:0>2}:00", minute).as_str();
                                        let datetime = date + "T" + time.as_str();
                                        datetime
                                    }
                                }; 
                                if let Some(ref mut item_list_unwrap) = item_list {
                                    schedule_item(&mut db, pos_int, time_start, unwrapped_list.id, item_list_unwrap)?;
                                }
                            }
                        }
                        else { println!("\tError While Parsing Position as usize"); }
                    },
                    "mark" => {
                        let item_pos = prompt_usr("Enter Item Position? ".to_string());
                        if let Ok(pos_int) = item_pos.parse::<usize>() {
                            let item_list_len = item_list.clone().unwrap().len();
                            if pos_int < item_list_len {
                                if let Some(ref mut item_list_unwrap) = item_list {
                                    mark_item(&mut db, pos_int, unwrapped_list.id, item_list_unwrap)?;
                                }
                            }
                        }
                        else { println!("\tError While Parsing Position as usize"); }
                    },
                    "update" => {
                        item_list = update_items(&mut db, unwrapped_list)?;
                    },
                    "hide" => {
                        hide_complete(&mut db, unwrapped_list)?;
                    },
                    "print" => { print_items(unwrapped_list, &item_list.clone().unwrap()); },
                    "list" => { println!("\tMoving To List Commands.."); break 'item; },
                    "exit" => { println!("\tExiting.."); break 'list; },
                    _ => println!("\tUnknown Command"),
                }
            }
        }
    }
    let _ = db.close();
    Ok(())
}

// dev function to clean up asking for user input
fn prompt_usr(prompt: String) -> String {
    let mut input = String::new();
    print!("{}", prompt);
    io::stdout().flush().expect("print buffer failed flush");
    io::stdin().read_line(&mut input).expect(" input failed be read");
    input = input.trim().to_string();
    return input
}

fn initialize_db(db: &Connection) -> Result<(), rusqlite::Error> {
    db.execute_batch(
        "create table if not exists List(
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              name TEXT NOT NULL,
              check_boxes BOOLEAN NOT NULL DEFAULT(FALSE),
              archived BOOLEAN NOT NULL DEFAULT(FALSE),
              hide_complete BOOLEAN NOT NULL DEFAULT(FALSE),
              time_created DATETIME NOT NULL,
              time_edited DATETIME NOT NULL
            );
            create table if not exists Item(
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              complete BOOLEAN NOT NULL DEFAULT(FALSE),
              text TEXT NOT NULL,
              parent INTEGER,
              next INTEGER,
              list_id INTEGER NOT NULL,
              FOREIGN KEY(parent) REFERENCES ITEM(id) on delete SET NULL,
              FOREIGN KEY(next) REFERENCES ITEM(id) on delete RESTRICT,
              FOREIGN KEY(list_id) REFERENCES List(id) on delete CASCADE
            );
            create table if not exists Recurrence(
              id INTEGER PRIMARY KEY,
              period INTEGER NOT NULL,
              time_last DATETIME NOT NULL,
              FOREIGN KEY(id) REFERENCES Item(id) on delete CASCADE
            );
            create table if not exists Schedule(
              id INTEGER PRIMARY KEY,
              activation_date DATETIME NOT NULL,
              FOREIGN KEY(id) REFERENCES Item(id) on delete CASCADE
            );
            PRAGMA foreign_keys = ON;",
    )?;
    Ok(())
}

// query the db for a desired lists information (id, name, completion mode, and item count)
fn get_list_info(db: &Connection, list_id: i32) -> Result<List, rusqlite::Error> {
    db.query_one(
        "select l.id, l.name, l.check_boxes, l.archived, l.hide_complete, l.time_created, l.time_edited, count(i.list_id) from List as l
            left join Item as i on l.id = i.list_id
            where l.id like ?1
            group by i.list_id",
        params![list_id],
        |row| {
            Ok(List { id: row.get(0)?, name: row.get(1)?, check_boxes: row.get(2)?, archived: row.get(3)?, hide_complete: row.get(4)?, 
                time_created: row.get(5)?, time_edited: row.get(6)?, item_count: row.get(7)? })
        }
    )
}

fn add_list(db: &Connection, list_name: String, check_boxes: bool) -> Result<(), rusqlite::Error> {
    let _list_inserted = db.execute(
        "insert into List(name, check_boxes, time_created, time_edited) values(?1, ?2, ?3, ?3)", 
        params![list_name, check_boxes, Local::now().timestamp()]
    )?;
    Ok(())
}

fn edit_list(db: &Connection, list_id: i32) -> Result<(), rusqlite::Error> {
    let _list_selected = db.query_one(
        "select name from List where id like ?1",
        params![list_id],
        |row| row.get::<usize, String>(0)
    )?;
    Ok(())
}

fn toggle_list_boxes(db: &Connection, list_id: i32) -> Result<(), rusqlite::Error> {
    // sqlite bools are 0 or 1, 1-1=0 true->false, |0-1|=|-1|=1 false->true
    let _list_toggled = db.execute(
        "update List set check_boxes = abs(check_boxes - 1) where id like ?1",
        params![list_id]
    )?;
    Ok(())
}

fn archive_list(db: &Connection, list_id: i32) -> Result<(), rusqlite::Error> {
    // sqlite bools are 0 or 1, 1-1=0 true->false, |0-1|=|-1|=1 false->true
    let _list_toggled = db.execute(
        "update List set archived = abs(archived - 1) where id like ?1",
        params![list_id]
    )?;
    Ok(())
}

fn delete_list(db: &Connection, list_id: i32) -> Result<(), rusqlite::Error> {
    let _list_deleted = db.execute(
        "delete from List where id like ?1", 
        params![list_id]
    )?;
    Ok(())
}

fn print_lists(db: &Connection) -> Result<(), rusqlite::Error> {
    let mut stmt = db.prepare(
        "select l.id, l.name, l.check_boxes, l.archived, l.hide_complete, l.time_created, l.time_edited, count(i.list_id)
            from List as l left join Item as i on l.id = i.list_id group by l.id"
    )?;
    let lists = stmt.query_map([],
        |row| {
            Ok(List { id: row.get(0)?, name: row.get(1)?, check_boxes: row.get(2)?, archived: row.get(3)?, hide_complete: row.get(4)?,
                time_created: row.get(5)?, time_edited: row.get(6)?, item_count: row.get(7)?, })
        }
    )?;
    let mut has_lists = false;
    for list in lists {
        has_lists = true;
        if let Ok(ref valid_list) = list {
            let formatted_created = format!("{}", DateTime::from_timestamp_secs(valid_list.time_created).unwrap().with_timezone(&Local).format("%Y/%m/%d"));
            let formatted_edited = format!("{}", DateTime::from_timestamp_secs(valid_list.time_edited).unwrap().with_timezone(&Local).format("%Y/%m/%d %H:%M:%S"));
            println!("\t{0}  {1}\tcompletion: {2}, archived: {3}, created: {4}, edited: {5}, count: {6}", 
                valid_list.id, valid_list.name, valid_list.check_boxes, valid_list.archived,
                formatted_created, formatted_edited, valid_list.item_count);
        }
    }
    if !has_lists { println!("\tNo Lists In File.."); }
    Ok(())
}

// execute transaction in db to update changed completion or schedule status, then rebuild the
//  memory list
fn update_items(db: &Connection, list_info: &List) -> Result<Option<Vec<Item>>, rusqlite::Error> {
    let _update = db.execute_batch("
        begin;
        --update the time edited for lists that have items updated
        update List as l
          set time_edited = unixepoch('now')
          where exists (select * from Item as i 
            join (Recurrence as r full outer join Schedule as s on r.id = s.id) 
              on i.id = (r.id or s.id)
            where l.id = i.list_id and 
              (unixepoch('now') - r.time_last > r.period 
              or unixepoch('now') > s.activation_date));
        --update the completed status and the time last
        update Item
          set complete = FALSE
          where exists (select period, time_last from Recurrence where id = Item.id
            and unixepoch('now') - time_last > period);
        --update the time_last of the recurrences
        update Recurrence 
          set time_last = case
            when period like 0 then unixepoch('now')
            else time_last + (period * floor((unixepoch('now') - time_last) / period))
            end
          where unixepoch('now') - time_last > period;
        --remove item from schedule after activation date
        delete from Schedule
          where unixepoch('now') - activation_date > 0;
        end;
    ")?;
    return build_ordered_items(&db, list_info.id)
}

// query the db for items of a list and generate the ordered list of items
fn build_ordered_items(db: &Connection, id: i32) -> Result<Option<Vec<Item>>, rusqlite::Error> {
    let stmt = db.prepare(
        "select i.id, i.complete, i.text, i.parent, i.next, r.period, r.time_last, s.activation_date
            from Item as i left join Recurrence as r on i.id = r.id
                left join Schedule as s on i.id = s.id
            where list_id like ?1"
    )?;
    let unsorted_list = get_unsorted_items(stmt, id)?;
    return Ok(Some(get_sorted_items(unsorted_list)))
}
fn get_unsorted_items(mut stmt: rusqlite::Statement, list_id: i32) -> Result<Vec<Item>, rusqlite::Error> {
    // map the queried rows to Item structs
    let items = stmt.query_map(params![list_id], |row| {
        let recurrence = {
            if let Ok(period) = row.get(5) && let Ok(time_last) = row.get(6) {
                Some(Recurrence { period, time_last })
            }
            else { None::<Recurrence> }
        };
        Ok(Item { id: row.get(0)?, complete: row.get(1)?, text: row.get(2)?, parent: row.get::<usize, Option<i32>>(3)?,
            next: row.get::<usize, Option<i32>>(4)?, recurrence, schedule_date: row.get::<usize, Option<i64>>(7)? })
    })?;
    // interate over the MappedRows and return them as a Vec
    let mut unsorted_list = vec![];
    for item in items {
        let valid_item = item?;
        unsorted_list.push(valid_item);
    }
    Ok(unsorted_list)
}
fn get_sorted_items(mut unsorted_list: Vec<Item>) -> Vec<Item> {
    let mut sorted_list: Vec<Item> = vec![];
    let mut i = 0;
    // iterate over the unsorted items looking for item with a next value equal to the id of the
    //  previously sorted item (or next of none if there are no sorted items)
    while unsorted_list.len() > 0 {
        if sorted_list.len() > 0 && unsorted_list[i].next == Some(sorted_list[sorted_list.len()-1].id) || unsorted_list[i].next == None {
            let matched_item = unsorted_list.remove(i);
            sorted_list.push(matched_item);
            if unsorted_list.len() != 0 {
                i %= unsorted_list.len();
            }
        }
        else {
            i = (i + 1) % unsorted_list.len();
        }
    }
    // correction for list being built in reverse
    sorted_list.reverse();
    sorted_list
}

// insert an item at the desired location, and update the next value for the item previously
//  pointing to the location
fn add_item(db: &mut Connection, position: usize, text: String, next_id: Option<i32>, list_id: i32, item_list: &mut Vec<Item>) -> Result<(), rusqlite::Error> {
    let tx = db.transaction()?;
    let _time_edited_updated = tx.execute(
        "update List set time_edited = unixepoch('now') where id like ?1",
        params![list_id]
    )?;
    let _item_inserted = tx.execute(
        "insert into Item(text, parent, next, list_id) values(?1, ?2, ?3, ?4)", 
        params![text, None::<i32>, next_id, list_id]
    )?;
    let inserted_id = tx.query_one(
        "select LAST_INSERT_ROWID()", [],
        |row| Ok(row.get::<usize, i32>(0)?)
    )?;
    let _item_updated = {
        if let None = next_id {
            tx.execute(
                "update Item set next = ?1 where next is null and id not like ?1 and list_id like ?3",
                params![inserted_id, next_id, list_id]
            )
        }
        else {
            tx.execute(
                "update Item set next = ?1 where next like ?2 and id not like ?1",
                params![inserted_id, next_id]
            )
        }
    }?;
    // update memory list
    item_list.insert(position, Item{ id: inserted_id, complete: false, text: text, parent: None, next: next_id, recurrence: None, schedule_date: None });
    if position > 0 {
        item_list[position-1].next = Some(inserted_id);
    }
    tx.commit()?;
    Ok(())
}

// update the text for an item
fn edit_item(db: &mut Connection, position: usize, new_text: String, list_id: i32, item_list: &mut Vec<Item>) -> Result<(), rusqlite::Error> {
    let tx = db.transaction()?;
    let _time_edited_updated = tx.execute(
        "update List set time_edited = unixepoch('now') where id like ?1",
        params![list_id]
    )?;
    let _time_edited_updated = tx.execute(
        "update List set time_edited = unixepoch('now') where id like ?1",
        params![list_id]
    )?;
    let _item_updated = tx.execute(
        "update Item set text = ?2 where id like ?1",
        params![item_list[position].id, new_text]
    )?;
    item_list[position].text = new_text;
    tx.commit()?;
    Ok(())
}

// removes item from the list, after moving the next value of the item pointing to the deleted item
//  to the deleted item's next, change parent of the children to None, returning Err(()) if the
//  transaction cannot be complted after altering memory list
fn delete_item(db: &mut Connection, position: usize, list_id: i32, item_list: &mut Vec<Item>) -> Result<(), rusqlite::Error> {
    let tx = db.transaction()?;
    let _time_edited_updated = tx.execute(
        "update List set time_edited = unixepoch('now') where id like ?1",
        params![list_id]
    )?;
    let _item_updated = tx.execute(
        "update Item set next = ?2 where next like ?1",
        params![item_list[position].id, item_list[position].next]
    )?;
    let _item_deleted = tx.execute(
        "delete from Item where id like ?1",
        params![item_list[position].id]
    )?;
    if position > 0 {
        item_list[position-1].next = item_list[position].next;
    }
    item_list.remove(position);
    let _update_deledted_children = update_parents(&tx, &position, None, item_list)?;    // correct children in new position if changed item is not indented
    tx.commit()?;
    Ok(())
}

// moves item at position old to position new, correcting next values to preserve integrity of order,
//  updating parent of children where nessesary, returning Err(()) if the transaction cannot be
//  completed after altering memory list
fn move_item(db: &mut Connection, pos_old: usize, pos_new: usize, list_id: i32, item_list: &mut Vec<Item>) -> Result<(), rusqlite::Error> {
    // item not moving, return
    if pos_old == pos_new { return Ok(()) }
    let tx = db.transaction()?;
    let _time_edited_updated = tx.execute(
        "update List set time_edited = unixepoch('now') where id like ?1",
        params![list_id]
    )?;
    move_update_order(&tx, pos_old, pos_new, item_list)?;
    // update children
    if let None = item_list[pos_new].parent {
        move_update_children(&tx, pos_old, pos_new, item_list)?;
    }
    tx.commit()?;
    Ok(())
}
fn move_update_order(tx: &Transaction, pos_old: usize, pos_new: usize, item_list: &mut Vec<Item>) -> Result<(), rusqlite::Error> {
    let _next_to_old_updated = tx.execute(
        "update Item set next = ?1 where next like ?2",
        params![item_list[pos_old].next, item_list[pos_old].id]
    )?;
    // get the where clause to select the item before the move location
    let where_next = {
        if pos_old < pos_new { item_list[pos_new].next }
        else { Some(item_list[pos_new].id) }
    };
    let _next_to_new_updated = tx.execute(
        "update Item set next = ?1 where next like ?2",
        params![item_list[pos_old].id, where_next]
    )?;
    // get the new parent of the moved item
    let parent = {
        let pos_parent = {
            if pos_old < pos_new { pos_new + 1 }
            else { pos_new }
        };
        if let Some(_) = item_list[pos_old].parent && pos_new > 0 { Some(item_list[get_parent(pos_parent, item_list)].id) }
        else { None::<i32> }
    };
    // get the new next for the moved item
    let set_next = {
        if pos_old < pos_new { item_list[pos_new].next }
        else { Some(item_list[pos_new].id) }
    };
    let _item_moved = tx.execute(
        "update Item set parent = ?2, next = ?3 where id like ?1",
        params![item_list[pos_old].id, parent, set_next]
    )?;
    // update memory list
    if pos_old > 0 { item_list[pos_old-1].next = item_list[pos_old].next; }
    item_list[pos_old].parent = parent;
    item_list[pos_old].next = set_next;
    let item = item_list.remove(pos_old);
    item_list.insert(pos_new, item);
    if pos_new > 0 { item_list[pos_new-1].next = Some(item_list[pos_new].id); }
    Ok(())
}
fn move_update_children(tx: &Transaction, pos_old: usize, pos_new: usize, item_list: &mut Vec<Item>) -> Result<(), rusqlite::Error> {
    // get the closes parent to the old location
    let pos_parent = move_get_parent(tx, pos_old, item_list)?;
    // correct children at old position if changed item was not indented
    if pos_old < pos_new {
        let mut pos_change = pos_old;
        if pos_change == 0 { pos_change = 1; }
        let update_old_children_stop = update_parents(&tx, &pos_change, Some(item_list[pos_parent].id), item_list)?;
        if update_old_children_stop < pos_new { 
            pos_change = pos_new + 1;
            let _update_new_children_children = update_parents(&tx, &pos_change, Some(item_list[pos_new].id), item_list)?;
        }
    }
    else {
        let mut pos_change = pos_new + 1;
        let update_new_children_stop = update_parents(&tx, &pos_change, Some(item_list[pos_new].id), item_list)?;
        if update_new_children_stop <= pos_new { 
            pos_change = pos_old;
            let _update_old_children = update_parents(&tx, &pos_change, Some(item_list[pos_parent].id), item_list)?;
        }
    }
    Ok(())
}
fn move_get_parent(tx: &Transaction, pos_old: usize, item_list: &mut Vec<Item>) -> Result<usize, rusqlite::Error> {
    if pos_old > 0 {
        Ok(get_parent(pos_old, item_list))
    }
    // the old location is 0, ensure the new item in that position has no parent
    //  and return position 0
    else {
        let _zero_parent_updated = tx.execute(
            "update Item set parent = Null where id like ?1",
            params![item_list[0].id]
        )?;
        item_list[0].parent = None;
        Ok(0)
    }
}

// binary opteration, adds a parent if item has none, removes parent if one exists
fn indent_item(db: &mut Connection, position: usize, list_id: i32, item_list: &mut Vec<Item>) -> Result<(), rusqlite::Error> {
    let tx = db.transaction()?;
    // item already indented, unindent
    let _time_edited_updated = tx.execute(
        "update List set time_edited = unixepoch('now') where id like ?1",
        params![list_id]
    )?;
    if let Some(_) = item_list[position].parent {
        let _indent_deleted = tx.execute(
            "update Item set parent = Null where id like ?1",
            params![item_list[position].id]
        )?;
        item_list[position].parent = None;
    }
    // check if item is at position 0, then indent
    else if position > 0 {
        let pos_parent = get_parent(position, item_list);
        let _indent_added = tx.execute(
            "update item set parent = ?2 where id like ?1",
            params![item_list[position].id, item_list[pos_parent].id]
        )?;
        item_list[position].parent = Some(item_list[pos_parent].id);
    }
    tx.commit()?;
    Ok(())
}

fn recur_item(db: &mut Connection, position: usize, period: i64, start: String, list_id: i32, item_list: &mut Vec<Item>) -> Result<(), rusqlite::Error> {
    let tx = db.transaction()?;
    let _time_edited_updated = tx.execute(
        "update List set time_edited = unixepoch('now') where id like ?1",
        params![list_id]
    )?;
    if start == "remove" {
        let _recurrence_deleted = tx.execute(
            "delete from Recurrence where id like ?1",
            params![item_list[position].id]
        )?;
        item_list[position].recurrence = None;
    }
    else {
        let datetime = {
            if let Ok(parsed_datetime) = start.parse::<NaiveDateTime>() {
                Local::from_local_datetime(&Local, &parsed_datetime).unwrap()
            }
            else {
                //NOTE not sure if this is how i want to handle this (may error and return?)
                //  double NOTE start == "now" will not parse, then datetime = Local::now()
                Local::now()
            }
        };
        let _recurrence_updated = tx.execute(
            "insert into Recurrence(id, period, time_last) values(?1, ?2, ?3)
                on conflict(id) do update set period = excluded.period, time_last = excluded.time_last",
            params![item_list[position].id, period, datetime.timestamp()]
        )?;
        item_list[position].recurrence = Some(Recurrence { period: period, time_last: datetime.timestamp() });
    }
    tx.commit()?;
    Ok(())
}

fn schedule_item(db: &mut Connection, position: usize, start: String, list_id: i32, item_list: &mut Vec<Item>) -> Result<(), rusqlite::Error> {
    let tx = db.transaction()?;
    let _time_edited_updated = tx.execute(
        "update List set time_edited = unixepoch('now') where id like ?1",
        params![list_id]
    )?;
    if start == "remove" {
        let _schdedule_deleted = tx.execute(
            "delete from Recurrence where id like ?1",
            params![item_list[position].id]
        )?;
        item_list[position].recurrence = None;
    }
    else {
        let datetime = {
            if let Ok(parsed_datetime) = start.parse::<NaiveDateTime>() {
                Local::from_local_datetime(&Local, &parsed_datetime).unwrap()
            }
            else {
                //NOTE not sure if this is how i want to handle this (may error and return?)
                //  double NOTE start == "now" will not parse, then datetime = Local::now()
                Local::now()
            }
        };
        let _schedule_updated = tx.execute(
            "insert into Schedule(id, activation_date) values(?1, ?2)
                on conflict(id) do update set activation_date = excluded.activation_date",
            params![item_list[position].id, datetime.timestamp()]
        )?;
        item_list[position].schedule_date = Some(datetime.timestamp());
    }
    tx.commit()?;
    Ok(())
}

// flip completion status
fn mark_item(db: &mut Connection, position: usize, list_id: i32, item_list: &mut Vec<Item>) -> Result<(), rusqlite::Error> {
    let tx = db.transaction()?;
    let _time_edited_updated = tx.execute(
        "update List set time_edited = unixepoch('now') where id like ?1",
        params![list_id]
    )?;
    // sqlite bools are 0 or 1, 1-1=0 true->false, |0-1|=|-1|=1 false->true
    let _item_updated = tx.execute(
        "update Item set complete = abs(complete - 1) where id like ?1",
        params![item_list[position].id]
    )?;
    item_list[position].complete = !item_list[position].complete;
    tx.commit()?;
    Ok(())
}

fn hide_complete(db: &mut Connection, list_info: &mut List) -> Result<(), rusqlite::Error> {
    let tx = db.transaction()?;
    let _time_edited_updated = tx.execute(
        "update List set time_edited = unixepoch('now') where id like ?1",
        params![list_info.id]
    )?;
    let _hide_toggled = tx.execute(
        "update List set hide_complete = abs(hide_complete - 1) where id like ?1",
        params![list_info.id]
    )?;
    list_info.hide_complete = !list_info.hide_complete;
    tx.commit()?;
    Ok(())
}

fn print_items(list_info: &List, item_list: &Vec<Item>) {
    if list_info.check_boxes {
        print_completion(list_info, item_list);
    }
    else {
        print_basic(item_list);
    }
}
// print two lists, uncompleted items and completed items
//  take into acount parent completion while displaying
//      * a completed parent should show all children as completed
//      * a completed child should show info on parent
fn print_completion(list_info: &List, item_list: &Vec<Item>) {
    let mut i = 0;
    let mut empty = true;
    let mut completed = vec![];
    let mut last_parent = None;
    let mut add_parent = false;
    println!("\tUn-Completed:");
    for item in item_list {
        if let None = item.parent { 
            last_parent = Some(IndexedItem{ index: i, item: item.clone() });
            add_parent = true;
        }
        if !item.complete {
            print_handle_uncompleted(item, i, &mut empty, &add_parent, &last_parent, &mut completed);
        }
        else {
            print_handle_completed(item, i, &mut add_parent, &last_parent, &mut completed);
        }
        i += 1;
    }
    if empty { println!("\t..."); }
    if !list_info.hide_complete {
        print_completed_items(completed);
    }
    else {
        println!("\tCompleted:");
        println!("\t--- HIDDEN");
    }
}
fn print_handle_uncompleted(item: &Item, index: usize, empty: &mut bool, add_parent: &bool, last_parent: &Option<IndexedItem>, completed: &mut Vec<IndexedItem>) {
    // if an uncompleted item has no parents it will be printed
    if let None = item.parent {
        print_item(item, index);
        *empty = false;
    }
    else {
        // if the last parent is complete and not added to the list, add it
        //  then add the current item
        if let Some(unwrapped_parent) = last_parent && unwrapped_parent.item.complete {
            if *add_parent {
                completed.push(IndexedItem { index: unwrapped_parent.clone().index, item: unwrapped_parent.clone().item });
            }
            completed.push(IndexedItem { index: index, item: item.clone() });
        }
        // the item and its parent are uncompleted so it may be printed
        else {
            print_item(item, index);
            *empty = false;
        }
    }
}
fn print_handle_completed(item: &Item, index: usize, add_parent: &mut bool, last_parent: &Option<IndexedItem>, completed: &mut Vec<IndexedItem>) {
    // the item is complete, so add the parent if it exists and is not in the list and the
    //  current item
    if let None = item.parent { *add_parent = false; }
    if let Some(unwrapped_parent) = last_parent && *add_parent  {
        completed.push(IndexedItem { index: unwrapped_parent.clone().index, item: unwrapped_parent.clone().item });
        *add_parent = false;
    }
    completed.push(IndexedItem { index: index, item: item.clone() });
}
fn print_completed_items(completed: Vec<IndexedItem>) {
    let mut empty = true;
    println!("\tCompleted:");
    for indexed_item in completed {
        if let None = indexed_item.item.parent && !indexed_item.item.complete {
            println!("\t--- {}", indexed_item.item.text);
        }
        else {
            print_item(&indexed_item.item, indexed_item.index);
            empty = false;
        }
    }
    if empty { println!("\t..."); }
}
fn print_basic(item_list: &Vec<Item>) {
    let mut i = 0;
    for item in item_list {
        print_item(item, i);
        i += 1;
    }
    if i == 0 {
        println!("No Items In List..");
    }
}
fn print_item(item: &Item, index: usize) {
    print!("\t");
    if let Some(_) = item.parent { print!("  "); }
    //print!("{0}: {1:?}", index, item); // debug print
    print!("{0: >2}: {1}", index, item.text);
    if let Some(ref recurrence) = item.recurrence && let Some(next_reoccurence) = DateTime::from_timestamp_secs(recurrence.time_last + recurrence.period) {
        print!("\t\tRecurring: {}", next_reoccurence.with_timezone(&Local));
    }
    if let Some(ref schedule_date) = item.schedule_date && let Some(unwrapped_date) = DateTime::from_timestamp_secs(*schedule_date) {
        print!("\t\tScheduled: {}", unwrapped_date.with_timezone(&Local));
    }
    println!("");
}

// get the closest prior item that has no parent
fn get_parent(position: usize, item_list: &Vec<Item>) -> usize {
    let mut pos_parent = position -1;
    while let Some(_) = item_list[pos_parent].parent {
        pos_parent -= 1;
    }
    return pos_parent
}

// update a block of children's parents of a after a position
fn update_parents(tx: &Transaction, position: &usize, new_parent: Option<i32>, item_list: &mut Vec<Item>) -> Result<usize, rusqlite::Error> {
    let mut pos_change = *position;
    while pos_change < item_list.len() && let Some(_) = item_list[pos_change].parent {
        let _parent_updated = tx.execute(
            "update Item set parent = ?2 where id like ?1",
            params![item_list[pos_change].id, new_parent]
        )?;
        item_list[pos_change].parent = new_parent;
        pos_change += 1;
    }
    Ok(pos_change)
}
