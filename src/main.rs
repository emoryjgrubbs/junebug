use std::io::{self, Write};
use rusqlite::{Connection, Result, params};
use chrono::{DateTime, Local, TimeZone, Utc, NaiveDateTime, NaiveDate};

#[derive(Debug, Clone)]
struct List {
    id: i32,
    name: String,
    check_boxes: bool,
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
    let db = Connection::open(path)?;
    db.execute_batch(
        "
            create table if not exists List(
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              name TEXT UNIQUE NOT NULL,
              check_boxes BOOLEAN NOT NULL DEFAULT(FALSE)
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
              time_last DATETIME NOT NULL DEFAULT(datetime('now')),
              FOREIGN KEY(id) REFERENCES Item(id) on delete CASCADE
            );
            create table if not exists Schedule(
              id INTEGER PRIMARY KEY,
              activation_date DATETIME NOT NULL,
              FOREIGN KEY(id) REFERENCES Item(id) on delete CASCADE
            );
            PRAGMA foreign_keys = ON;
            begin;
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
        ",
    )?;
    let mut current_list: Option<List> = None;
    let mut item_list: Option<Vec<Item>> = None;
    'list: loop {
        let list_command = prompt_usr("Enter List Command? ".to_string());
        match list_command.to_lowercase().as_str() {
            "add" => {
                let list_name = prompt_usr("Enter List Name? ".to_string());
                let check_boxes_input = prompt_usr("Use Completion? [Y/n] ".to_string());
                // TODO should this be handled by prompt_usr?
                let check_boxes = {
                    if check_boxes_input.to_lowercase().as_str() == "" { "TRUE" }
                    else { "FALSE" }
                };
                let list_inserted = db.execute(
                    "insert into List(name, check_boxes) values(?1,?2)", 
                    params![list_name, check_boxes.trim()]
                );
                if let Ok(1) = list_inserted {
                    current_list = Some(get_list_info(list_name, &db)?);
                    item_list = Some(vec![]);
                }
                else {
                    println!("\tError Occured While Adding {}", list_name)
                }
            },
            "edit" => {
                let list_name = prompt_usr("Enter List Name? ".to_string());
                let list_selected = db.query_one(
                    "select name from List where name like ?1",
                    params![list_name],
                    |row| row.get::<usize, String>(0)
                );
                if let Ok(_) = list_selected {
                    current_list = Some(get_list_info(list_name, &db)?);
                    if let Some(ref unwrapped_list) = current_list {
                        item_list = update_items(&db, unwrapped_list);
                    }
                }
                else {
                    println!("\tError Occured While Editing {}", list_name)
                }
            },
            "delete" => {
                let list_name = prompt_usr("Enter List Name? ".to_string());
                let list_deleted = db.execute(
                    "delete from List where name like ?1", 
                    params![list_name]
                );
                if let Err(_) = list_deleted {
                    println!("\tError Occured While Deleting {}", list_name)
                }
                if let Some(ref unwrapped_list) = current_list {
                    if unwrapped_list.name == list_name {
                        current_list = None;
                        item_list = None;
                    }
                }
            },
            "print" => {
                let stmt = db.prepare("select name from List");
                if let Ok(mut prepared_stmt) = stmt {
                    let lists = prepared_stmt.query_map([], |row| row.get::<usize, String>(0));
                    if let Ok(lists_valid) = lists {
                        let mut has_lists = false;
                        for list in lists_valid {
                            has_lists = true;
                            println!("\t{}", list?);
                        }
                        if !has_lists { println!("\tNo Lists In File.."); }
                    }
                }
            },
            "exit" => { println!("\tExiting.."); break 'list; },
            _ => println!("\tUnknown Command"),
        }
        if let Some(ref unwrapped_list) = current_list {
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
                                    add_item(&db, pos_int, item_text, next, current_list.clone().unwrap().id, item_list_unwrap);
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
                                    edit_item(&db, pos_int, new_text, item_list_unwrap);
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
                                    if let Err(_) = delete_item(&db, pos_int, item_list_unwrap) {
                                        item_list = update_items(&db, unwrapped_list);
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
                                    if let Err(_) = move_item(&db, old_int, new_int, item_list_unwrap) {
                                        item_list = update_items(&db, unwrapped_list);
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
                                    indent_item(&db, pos_int, item_list_unwrap);
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
                                        Some("remove".to_string())
                                    }
                                    else {
                                        let start_now = prompt_usr("Start Recurrence Now? [Y/n] ".to_string());
                                        if start_now == "n" {
                                            let year = prompt_usr("Enter Start Year? ".to_string());
                                            let month = prompt_usr("Enter Start Month? ".to_string());
                                            let day = prompt_usr("Enter Start Day? ".to_string());
                                            let hour = prompt_usr("Enter Start Hour? ".to_string());
                                            let minute = prompt_usr("Enter Start Minute? ".to_string());
                                            //create date as yyyy-mm-dd
                                            let date = format!("{:0>4}-", year).to_owned() + format!("{:0>2}-", month).as_str() + format!("{:0>2}", day).as_str();
                                            //create time as hh:mm
                                            let time = format!("{:0>2}:", hour).to_owned() + format!("{:0>2}:00", minute).as_str();
                                            let datetime = date + "T" + time.as_str();
                                            if let Ok(_) = datetime.parse::<NaiveDateTime>() {
                                                Some(datetime)
                                            }
                                            else { None }
                                        }
                                        else { Some("now".to_string()) }
                                    }
                                }; 
                                let period = {
                                    if let Some(_) = time_start && time_start != Some("remove".to_string()) {
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
                                if let Some(ref mut item_list_unwrap) = item_list {
                                    recur_item(&db, pos_int, period, time_start, item_list_unwrap);
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
                                        Some("remove".to_string())
                                    }
                                    else {
                                        let year = prompt_usr("Enter Schedule Year? ".to_string());
                                        let month = prompt_usr("Enter Schedule Month? ".to_string());
                                        let day = prompt_usr("Enter Schedule Day? ".to_string());
                                        let hour = prompt_usr("Enter Schedule Hour? ".to_string());
                                        let minute = prompt_usr("Enter Schedule Minute? ".to_string());
                                        //create date as yyyy-mm-dd
                                        let date = format!("{:0>4}-", year).to_owned() + format!("{:0>2}-", month).as_str() + format!("{:0>2}", day).as_str();
                                        //create time as hh:mm
                                        let time = format!("{:0>2}:", hour).to_owned() + format!("{:0>2}:00", minute).as_str();
                                        let datetime = date + "T" + time.as_str();
                                        if let Ok(_) = datetime.parse::<NaiveDateTime>() {
                                            Some(datetime)
                                        }
                                        else { None }
                                    }
                                }; 
                                if let Some(ref mut item_list_unwrap) = item_list {
                                    schedule_item(&db, pos_int, time_start, item_list_unwrap);
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
                                    mark_item(&db, pos_int, item_list_unwrap);
                                }
                            }
                        }
                        else { println!("\tError While Parsing Position as usize"); }
                    },
                    "update" => {
                        item_list = update_items(&db, unwrapped_list);
                    },
                    "print" => { 
                        print_items(unwrapped_list, &item_list.clone().unwrap());
                    },
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

//dev function to clean up asking for user input
fn prompt_usr(prompt: String) -> String {
    let mut input = String::new();
    print!("{}", prompt);
    io::stdout().flush().expect("print buffer failed flush");
    io::stdin().read_line(&mut input).expect(" input failed be read");
    input = input.trim().to_string();
    return input
}

//query the db for the row of the desired list and return it
fn get_list_info(list_name: String, db: &Connection) -> Result<List, rusqlite::Error> {
    db.query_one(
        "select * from List where name like ?1",
        params![list_name],
        |row| {
            let check_boxes;
            if Ok("TRUE".to_string()) == row.get(2) { check_boxes = true; }
            else { check_boxes = false; }
            Ok(List { id: row.get(0)?, name: row.get(1)?, check_boxes})
        }
    )
}

//query the db for items of a list and generate the ordered list of items
fn build_ordered_items(db: &Connection, id: i32) -> Vec<Item> {
    let stmt = db.prepare(
        "select i.id, i.complete, i.text, i.parent, i.next, r.period, r.time_last, s.activation_date
            from Item as i left join Recurrence as r on i.id = r.id
                left join Schedule as s on i.id = s.id
            where list_id like ?1"
    );
    if let Ok(mut prepared_stmt) = stmt {
        let items = prepared_stmt.query_map(params![id], |row| {
            //println!("{:?}", row.get::<usize, bool>(1)?);
            let parent = {
                if let Ok(p_id) = row.get(3) { Some(p_id) }
                else { None::<i32> }
            };
            let next = {
                if let Ok(n_id) = row.get(4) { Some(n_id) }
                else { None::<i32> }
            };
            let recurrence = {
                if let Ok(period) = row.get(5) && let Ok(time_last) = row.get(6) {
                    Some(Recurrence { period, time_last })
                }
                else { None::<Recurrence> }
            };
            let schedule_date = {
                if let Ok(date) = row.get(7) {
                    Some(date)
                }
                else { None::<i64> }
            };
            Ok(Item { id: row.get(0)?, complete: row.get(1)?, text: row.get(2)?, parent, next , recurrence, schedule_date })
        });
        if let Ok(items_valid) = items {
            let mut unsorted_list = vec![];
            let mut sorted_list = vec![];
            //find item with no next (end of list)
            for item in items_valid {
                let expected_item = item.expect("query returned Ok");
                if expected_item.next == None {
                    sorted_list.push(expected_item.clone());
                }
                else {
                    unsorted_list.push(expected_item);
                }
            }
            //iterate over the unsorted items until all have been pushed to the sorted list
            let mut i = 0;
            while unsorted_list.len() > 0 {
                if unsorted_list[i].next == Some(sorted_list[sorted_list.len()-1].id) {
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
            //correction for list being built in reverse
            sorted_list.reverse();
            return sorted_list
        }
    }
    return vec![]
}

//insert an item at the desired location, next value for the item pointing to old item at location
//is updated
fn add_item(db: &Connection, position: usize, text: String, next_id: Option<i32>, list_id: i32, item_list: &mut Vec<Item>) {
    let _ = db.execute("begin", ());
    let item_inserted = db.execute(
        "insert into Item(text, parent, next, list_id) values(?1, ?2, ?3, ?4)", 
        params![text, None::<i32>, next_id, list_id]
    );
    let item_inserted_id = db.query_one(
        "select LAST_INSERT_ROWID()", [],
        |row| Ok(row.get::<usize, i32>(0)?)
    );
    if let Ok(_) = item_inserted && let Ok(valid_id) = item_inserted_id {
        let item_updated = {
            if let None = next_id {
                db.execute(
                    "update Item set next = ?1 where next is null and id not like ?1 and list_id like ?3",
                    params![valid_id, next_id, list_id]
                )
            }
            else {
                db.execute(
                    "update Item set next = ?1 where next like ?2 and id not like ?1",
                    params![valid_id, next_id]
                )
            }
        };
        if let Ok(_) = item_updated {
            let _ = db.execute("end", []);
            //update memory list
            item_list.insert(position, Item{ id: valid_id, complete: false, text: text, parent: None, next: next_id, recurrence: None, schedule_date: None });
            if position > 0 {
                item_list[position-1].next = Some(valid_id);
            }
        }
        else {
            let _ = db.execute("rollback", []);
            println!("\tError While Updating Item Order");
        }
    }
    else {
        let _ = db.execute("rollback", []);
        println!("\tError While Inserting Item");
    }
}

//update the text for an item
fn edit_item(db: &Connection, position: usize, new_text: String, item_list: &mut Vec<Item>) {
    let item_updated = db.execute(
        "update Item set text = ?2 where id like ?1",
        params![item_list[position].id, new_text]
    );
    if let Ok(_) = item_updated {
        item_list[position].text = new_text;
    }
}

//removes item from the list, after moving the next value of the item pointing to the deleted item
//to the deleted item's next, change parent of the children to None, returning Err(()) if the
//transaction cannot be complted after altering memory list
fn delete_item(db: &Connection, position: usize, item_list: &mut Vec<Item>) -> Result<(), ()> {
    let _ = db.execute("begin", ());
    let item_updated = db.execute(
        "update Item set next = ?2 where next like ?1",
        params![item_list[position].id, item_list[position].next]
    );
    let item_deleted = db.execute(
        "delete from Item where id like ?1",
        params![item_list[position].id]
    );
    if let Ok(_) = item_deleted && let Ok(_) = item_updated {
        let _ = db.execute("end", []);
        if position > 0 {
            item_list[position-1].next = item_list[position].next;
        }
        item_list.remove(position);
        if let Err(_) = update_parents(db, &position, None, item_list) {
           let _ = db.execute("rollback", []);
           return Err(())
        }
        let _ = db.execute("end", []);
    }
    else { let _ = db.execute("rollback", []); }
    Ok(())
}

//moves item at position old to position new, correcting next values to preserve integrity of order,
//updating parent of children where nessesary, returning Err(()) if the transaction cannot be
//completed after altering memory list
fn move_item(db: &Connection, pos_old: usize, pos_new: usize, item_list: &mut Vec<Item>) -> Result<(), ()> {
    if pos_old == pos_new { return Ok(()) }
    // if parent, find parent in new pos
    let _ = db.execute("begin", ());
    let next_to_old_updated = db.execute(
        "update Item set next = ?1 where next like ?2",
        params![item_list[pos_old].next, item_list[pos_old].id]
    );
    let where_next = {
        if pos_old < pos_new { item_list[pos_new].next }
        else { Some(item_list[pos_new].id) }
    };
    let next_to_new_updated = db.execute(
        "update Item set next = ?1 where next like ?2",
        params![item_list[pos_old].id, where_next]
    );
    let parent = {
        let pos_parent = {
            if pos_old < pos_new { pos_new + 1 }
            else { pos_new }
        };
        if let Some(_) = item_list[pos_old].parent && pos_new > 0 { Some(item_list[get_parent(pos_parent, item_list)].id) }
        else { None::<i32> }
    };
    let set_next = {
        if pos_old < pos_new { item_list[pos_new].next }
        else { Some(item_list[pos_new].id) }
    };
    let item_moved = db.execute(
        "update Item set parent = ?2, next = ?3 where id like ?1",
        params![item_list[pos_old].id, parent, set_next]
    );
    if let Ok(_) = item_moved && let Ok(_) = next_to_old_updated && let Ok(_) = next_to_new_updated {
        // update memory list
        if pos_old > 0 { item_list[pos_old-1].next = item_list[pos_old].next; }
        item_list[pos_old].parent = parent;
        item_list[pos_old].next = set_next;
        let item = item_list.remove(pos_old);
        item_list.insert(pos_new, item);
        if pos_new > 0 { item_list[pos_new-1].next = Some(item_list[pos_new].id); }
        if let None = item_list[pos_new].parent {
            //get the closest parent prior to the moved item
            //  making the new 0 item the parent if needed
            let pos_parent = {
                if pos_old > 0 {
                    get_parent(pos_old, item_list)
                }
                else {
                    let zero_updated = db.execute(
                        "update Item set parent = Null where id like ?1",
                        params![item_list[0].id]
                    );
                    if let Err(_) = zero_updated {
                        let _ = db.execute("rollback", []);
                        println!("\tError While Ensuring Item Zero Has No Parent");
                        return Err(())
                    }
                    item_list[0].parent = None;
                    0
                }
            };
            //correct children at old position if changed item was not indented
            let mut pos_change = pos_old;
            if pos_change == 0 { pos_change = 1; }
            if let Err(_) = update_parents(&db, &pos_change, Some(item_list[pos_parent].id), item_list) {
                println!("\tError While Updating Children Of Moved Item");
                return Err(())
            }
            //correct children in new position if changed item is not indented
            pos_change = pos_new;
            if pos_old < pos_new { pos_change += 1; }
            if let Err(_) = update_parents(&db, &pos_change, Some(item_list[pos_new].id), item_list) {
                println!("\tError While Updating Children Of Destination");
                return Err(())
            }
        }
        let _ = db.execute("end", []);
    }
    else {
        let _ = db.execute("rollback", []);
        println!("\tError While Moving Item");
    }
    Ok(())
}

//binary opteration, adds a parent if item has none, removes parent if one exists
fn indent_item(db: &Connection, position: usize, item_list: &mut Vec<Item>) {
    if let Some(_) = item_list[position].parent {
        let indent_deleted = db.execute(
            "update Item set parent = Null where id like ?1",
            params![item_list[position].id]
        );
        if let Ok(_) = indent_deleted { item_list[position].parent = None; }
        else { println!("\tError While Deleting Item Indent"); }
    }
    else if position > 0 {
        let _ = db.execute("begin", ());
        let pos_parent = get_parent(position, item_list);
        let indent_added = db.execute(
            "update item set parent = ?2 where id like ?1",
            params![item_list[position].id, item_list[pos_parent].id]
        );
        if let Ok(_) = indent_added {
            let _ = db.execute("end", []);
            item_list[position].parent = Some(item_list[pos_parent].id);
        }
        else { println!("\tError While Adding Item Indent"); }
    }
}

fn recur_item(db: &Connection, position: usize, period: Option<i64>, start: Option<String>, item_list: &mut Vec<Item>) {
    if let Some(unwrapped_start) = start {
        if unwrapped_start == "remove" {
            let recurrence_deleted = db.execute(
                "delete from Recurrence where id like ?1",
                params![item_list[position].id]
            );
            if let Ok(_) = recurrence_deleted {
                item_list[position].recurrence = None;
            }
            else { println!("\tError While Deleting Item Recurrence"); }
        }
        else {
            if let Some(unwrapped_period) = period {
                let datetime = {
                    if unwrapped_start == "now" {
                        Local::now()
                    }
                    else {
                        let a = unwrapped_start.parse::<NaiveDateTime>().expect("validitiy check by caller, and successfully inserted");
                        Local::from_local_datetime(&Local, &a).unwrap()
                    }
                };
                let recurrence_updated = db.execute(
                    "insert into Recurrence(id, period, time_last) values(?1, ?2, ?3)
                        on conflict(id) do update set period = excluded.period, time_last = excluded.time_last",
                    params![item_list[position].id, unwrapped_period, datetime.timestamp()]
                );
                if let Ok(_) = recurrence_updated {
                    item_list[position].recurrence = Some(Recurrence { period: unwrapped_period, time_last: datetime.timestamp() });
                }
                else { println!("\tError While Updating Item Recurrence"); }
            }
        }
    }
}

fn schedule_item(db: &Connection, position: usize, start: Option<String>, item_list: &mut Vec<Item>) {
    if let Some(unwrapped_start) = start {
        if unwrapped_start == "remove" {
            let recurrence_deleted = db.execute(
                "delete from Recurrence where id like ?1",
                params![item_list[position].id]
            );
            if let Ok(_) = recurrence_deleted {
                item_list[position].recurrence = None;
            }
            else { println!("\tError While Deleting Item Recurrence"); }
        }
        else {
            let datetime = {
                if unwrapped_start == "now" {
                    Local::now()
                }
                else {
                    let a = unwrapped_start.parse::<NaiveDateTime>().expect("validitiy check by caller, and successfully inserted");
                    Local::from_local_datetime(&Local, &a).unwrap()
                }
            };
            let recurrence_updated = db.execute(
                "insert into Schedule(id, activation_date) values(?1, ?2)
                    on conflict(id) do update set activation_date = excluded.activation_date",
                params![item_list[position].id, datetime.timestamp()]
            );
            if let Ok(_) = recurrence_updated {
                item_list[position].schedule_date = Some(datetime.timestamp());
            }
            else { println!("\tError While Updating Item Recurrence"); }
        }
    }
}

//flip completion status
fn mark_item(db: &Connection, position: usize, item_list: &mut Vec<Item>) {
    // sqlite bools are 0 or 1, 1-1=0 true->false, |0-1|=|-1|=1 false->true
    let item_updated = db.execute(
        "update Item set complete = abs(complete - 1) where id like ?1",
        params![item_list[position].id]
    );
    if let Ok(_) = item_updated {
        item_list[position].complete = !item_list[position].complete
    }
    else {
        println!("\tError While Inserting Item");
    }
}

fn print_items(list_info: &List, item_list: &Vec<Item>) {
    if list_info.check_boxes {
        print_completion(item_list);
    }
    else {
        print_basic(item_list);
    }
}
fn print_completion(item_list: &Vec<Item>) {
    //take into acount parent completion while displaying
    //  a completed parent should show all children as completed
    //  a completed child should show info on parent
    let mut i = 0;
    let mut empty = true;
    let mut last_parent = None;
    let mut add_parent = false;
    let mut completed = vec![];
    println!("\tUn-Completed:");
    for item in item_list {
        // any item with no parent may be needed if a sub item is complete or it is complete as a
        //  parent
        if let None = item.parent { 
            last_parent = Some(IndexedItem{ index: i, item: item.clone() });
            add_parent = true;
        }
        if !item.complete {
            // if an uncompleted item has no parents it will be printed
            if let None = item.parent {
                print!("\t");
                if let Some(_) = item.parent { print!("  "); }
                print!("{0: >2}: {1}", i, item.text);
                if let Some(ref recurrence) = item.recurrence && let Some(next_reoccurence) = DateTime::from_timestamp_secs(recurrence.time_last + recurrence.period) {
                    print!("\t\tRecurring: {}", next_reoccurence.with_timezone(&Local));
                }
                if let Some(ref schedule_date) = item.schedule_date && let Some(unwrapped_date) = DateTime::from_timestamp_secs(*schedule_date) {
                    print!("\t\tScheduled: {}", unwrapped_date.with_timezone(&Local));
                }
                println!("");
                empty = false;
            }
            else {
                // if the last parent is complete and not added to the list, add it
                //  then add the current item
                if let Some(ref unwrapped_parent) = last_parent && unwrapped_parent.item.complete {
                    if add_parent {
                        completed.push(IndexedItem { index: unwrapped_parent.clone().index, item: unwrapped_parent.clone().item });
                    }
                    completed.push(IndexedItem { index:i, item: item.clone() });
                }
                // the item and its parent are uncompleted so it may be printed
                else {
                    print!("\t");
                    if let Some(_) = item.parent { print!("  "); }
                    print!("{0: >2}: {1}", i, item.text);
                    if let Some(ref recurrence) = item.recurrence && let Some(next_reoccurence) = DateTime::from_timestamp_secs(recurrence.time_last + recurrence.period) {
                        print!("\t\tRecurring: {}", next_reoccurence.with_timezone(&Local));
                    }
                    if let Some(ref schedule_date) = item.schedule_date && let Some(unwrapped_date) = DateTime::from_timestamp_secs(*schedule_date) {
                        print!("\t\tScheduled: {}", unwrapped_date.with_timezone(&Local));
                    }
                    println!("");
                    empty = false;
                }
            }
        }
        // the item is complete, so add the parent if it exists and is not in the list and the
        //  current item
        else {
            if let None = item.parent { add_parent = false; }
            if let Some(ref unwrapped_parent) = last_parent && add_parent  {
                completed.push(IndexedItem { index: unwrapped_parent.clone().index, item: unwrapped_parent.clone().item });
                add_parent = false;
            }
            completed.push(IndexedItem { index: i, item: item.clone() });
        }
        i += 1;
    }
    if empty { println!("\t..."); }
    empty = true;
    println!("\tCompleted:");
    for indexed_item in completed {
        if let None = indexed_item.item.parent && !indexed_item.item.complete {
            println!("\t--- {}", indexed_item.item.text);
        }
        else {
            print!("\t");
            if let Some(_) = indexed_item.item.parent { print!("  "); }
            print!("{0: >2}: {1}", indexed_item.index, indexed_item.item.text);
            if let Some(ref recurrence) = indexed_item.item.recurrence && let Some(next_reoccurence) = DateTime::from_timestamp_secs(recurrence.time_last + recurrence.period) {
                print!("\t\tRecurring: {}", next_reoccurence.with_timezone(&Local));
            }
            if let Some(ref schedule_date) = indexed_item.item.schedule_date && let Some(unwrapped_date) = DateTime::from_timestamp_secs(*schedule_date) {
                print!("\t\tScheduled: {}", unwrapped_date.with_timezone(&Local));
            }
            println!("");
            empty = false;
        }
    }
    if empty { println!("\t..."); }
}
fn print_basic(item_list: &Vec<Item>) {
    let mut i = 0;
    for item in item_list {
        print!("\t");
        if let Some(_) = item.parent { print!("  "); }
        print!("{0: >2}: {1}", i, item.text);
        if let Some(ref recurrence) = item.recurrence && let Some(next_reoccurence) = DateTime::from_timestamp_secs(recurrence.time_last + recurrence.period) {
            print!("\t\tRecurring: {}", next_reoccurence.with_timezone(&Local));
        }
        if let Some(ref schedule_date) = item.schedule_date && let Some(unwrapped_date) = DateTime::from_timestamp_secs(*schedule_date) {
            print!("\t\tScheduled: {}", unwrapped_date.with_timezone(&Local));
        }
        println!("");
        i += 1;
    }
    if i == 0 {
        println!("No Items In List..");
    }
}

//execute transaction in db to update changed completion or schedule status, then rebuild the
//memory list
fn update_items(db: &Connection, list_info: &List) -> Option<Vec<Item>> {
    let update = db.execute_batch("
        begin;
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
    ");
    if let Err(_) = update {
        println!("\tError While Updating Item Recurrence And Schedule Status");
    }
    return Some(build_ordered_items(&db, list_info.id))
}

//get the closest prior item that has no parent
fn get_parent(position: usize, item_list: &Vec<Item>) -> usize {
    let mut pos_parent = position -1;
    while let Some(_) = item_list[pos_parent].parent {
        pos_parent -= 1;
    }
    return pos_parent
}

//update a block of children's parents of a after a position
fn update_parents(db: &Connection, position: &usize, new_parent: Option<i32>, item_list: &mut Vec<Item>) -> Result<(), ()>{
    let mut pos_change = *position;
    while pos_change < item_list.len() && let Some(_) = item_list[pos_change].parent {
        item_list[pos_change].parent = new_parent;
        let parent_updated = db.execute(
            "update Item set parent = ?2 where id like ?1",
            params![item_list[pos_change].id, new_parent]
        );
        if let Err(_) = parent_updated {
            let _ = db.execute("rollback", []);
            println!("\tError While Correcting Subsequent Parents To Moved Item");
            return Err(())
        }
        pos_change += 1;
    }
    Ok(())
}
