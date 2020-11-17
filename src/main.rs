use std::process::{Command};
use std::io::{stdout, Write, ErrorKind};
use std::env;
use colored::*;
use text_io::read;
use std::path::{Path, PathBuf};
use term_size;
use ansi_escapes::*;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    Result,
};


// clear the command line
fn clear() {
    Command::new("cmd").args(&["/C", "cls"]).status();
}


// Change directories.
fn cd(args: Vec<&str>) {
    let dir;
    let home = env::var("HOMEPATH").unwrap();

    if args[1] == "~" {
        dir = Path::new(&home);
    } else {
        dir = Path::new(args[1]);
    }

    let result = env::set_current_dir(&dir);
    let result = match result {
        Ok(output) => output,
        Err(error) => match error.kind() {
            ErrorKind::NotFound => {
                println!("Could not cd to \"{}\": Directory not found.", args[1]);
            },
            ErrorKind::PermissionDenied => {
                println!("Could not cd to \"{}\": Access is denied.", args[1])
            }
            other => {
                println!("Could not CD to \"{}\": {:?}", args[1], other);
            }
        }
    };
}


// list directories
fn ls(args: Vec<&str>) {
    let mut term_width = 100;
    if let Some((w, h)) = term_size::dimensions() {
        term_width = w;
    } else {
        println!("Unable to obtain terminal size. Assuming 100.")
    }

    let cwd = match env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => return println!("Could not obtain current working directory: {:?}", error),
    };

    let files = match cwd.read_dir() {
        Ok(files) => files,
        Err(error) => return println!("Could not read directory: {:?}", error),
    };

    let mut charcount = 0;
    let mut perfect = true;

    for file in files {
        if let Ok(file) = file {
            let filename = file.file_name().into_string().unwrap();
            let meta = file.metadata().unwrap();
            let mut output = String::new();

            if (charcount + &filename.chars().count()) > term_width {
                output = format!("\n\n{}   ", &filename);
                charcount = filename.chars().count() + 3;
            } else if (charcount + &filename.chars().count()) == term_width {
                output = format!("{}\n\n", &filename);
                charcount = 0;
            } else {
                output = format!("{}   ", &filename);
                charcount = charcount + filename.chars().count() + 3;
            }

            if meta.is_dir() {
                print!("{}", output.bright_blue());
            } else if filename.ends_with(".lnk") {
                print!("{}", output.bright_cyan());
            } else {
                print!("{}", output.bright_green());
            }
        } else {
            perfect = false;
        }
    }
    if perfect {
        println!("");
    } else {
        println!("\nSome items omitted due to errors.");
    }
}

// obtain user@host:~$ or whatever it should be
fn get_prompt() -> String {
    let user = env::var("USERNAME").unwrap().to_lowercase();
    let host = env::var("COMPUTERNAME").unwrap().to_lowercase();

    let mut output = user + "@" + &host;
    output = output.bright_green().to_string();

    let home_abs = env::current_dir().unwrap().canonicalize().unwrap();
    let mut cwd = home_abs.into_os_string().into_string().unwrap();

    let home_rel_str = env::var("HOMEPATH").unwrap();
    let home_rel = Path::new(&home_rel_str);
    let home_abs = home_rel.canonicalize().unwrap();
    let home = home_abs.into_os_string().into_string().unwrap();

    if cwd.starts_with(&home) {
        cwd = cwd.replace(&home, "~");
    } else {
        cwd = env::current_dir().unwrap().into_os_string().into_string().unwrap();
    }
    cwd = cwd.replace("\\", "/");

    return format!("{}:{}{} ", output.bold(), cwd.bright_blue().bold(), "$".normal().clear());
}


// Oh, here comes the HITLER.
fn main() {
    // Clear out screen
    clear();

    // Set up history variables.
    let mut history = vec![String::new()];
    let mut history_pos = 0;

    // Drop into infinte loop
    loop {
        // prompt stores the user@host:~$ for the current command
        let prompt = get_prompt();

        // line stores the current prompt with the command being entered
        let mut line = prompt.clone();

        // min_pos stores the minimum position in the string.
        // (to prevent the user from editing the prompt)
        let min_pos = line.chars().count();

        // pos stores the current position in the command string
        let mut pos = min_pos;
        let mut stdout = stdout();

        // print out prompt
        print!("{}", line);
        stdout.flush();

        // then wait for keypress event
        while let Event::Key(KeyEvent { code, ..}) = event::read().unwrap() {
            match code {

                KeyCode::Backspace => {
                    // If the cursor is at the right end of the
                    // prompt, they can't go any further so BEEP.
                    if pos <= min_pos {
                        print!("{}", Beep);

                    // Otherwise, facilitate backspace behavior.
                    // Don't ask me, iunno how this works.
                    } else {
                        let (f, last) = line.split_at(pos);
                        let mut first = String::from(f);
                        first.pop();
                        line = format!("{}{}", first, last);
                        pos = pos - 1;
                        let back = (line.chars().count() - pos) as u16;
                        print!("{}{}{}", EraseLine, CursorLeft, line);
                        if back > 0 { print!("{}", CursorBackward(back)); }
                        let command = line.replace(&prompt, "");
                        let l = history.len();
                        history[l-1] = command;
                        if history[l-1] == "" { history.pop(); }
                    }
                }

                KeyCode::Left => {
                    // If at the end of the prompt, BEEP
                    if pos <= min_pos {
                        print!("{}", Beep);
                    // Otherwise move cursor backwards.
                    } else {
                        print!("{}", CursorBackward(1));
                        pos = pos - 1;
                    }
                }

                KeyCode::Right => {
                    // If at end of the command, BEEP
                    if pos >= line.chars().count() {
                        print!("{}", Beep);
                    // Otherwise move cursor forward.
                    } else {
                        print!("{}", CursorForward(1));
                        pos = pos + 1;
                    }
                }

                // If enter is pressed, the command is complete,
                // and we should break out of the loop to process it.
                KeyCode::Enter => {
                    break;
                }

                KeyCode::Up => {
                    // Facilitate history recall behavior, but ONLY if there's
                    // history available.
                    if history_pos > 0 {
                        history_pos = history_pos - 1;
                        line = format!("{}{}", prompt, history[history_pos]);
                        pos = line.chars().count();
                        print!("{}{}{}", EraseLine, CursorLeft, line);
                    }
                }

                // Same here, but backwards.
                KeyCode::Down => {
                    if history_pos < (history.len() - 1) {
                        history_pos = history_pos + 1;
                        line = format!("{}{}", prompt, history[history_pos]);
                        pos = line.chars().count();
                        print!("{}{}{}", EraseLine, CursorLeft, line);
                    }
                }

                // Ugh, and then there's THIS piece of shit.
                KeyCode::Tab => {
                    // So here we set some variables and...
                    let command_str = line.replace(&prompt, "");
                    let mut command: Vec<&str> = command_str.split(" ").collect();
                    let last = command[command.len()-1];
                    let cwd = env::current_dir().unwrap();
                    let files = cwd.read_dir().unwrap();
                    let mut matches: Vec<String> = Vec::new();

                    // You know what? It's magic. It's fucking magic.

                    for file in files {
                        let file_ref = file.as_ref();
                        let filename = file_ref.unwrap().file_name().to_os_string().into_string().unwrap();
                        if filename.starts_with(last) {
                            matches.push(filename);
                        }
                    }

                    if matches.len() == 0 {
                        print!("{}", Beep);
                    } else if matches.len() == 1 {
                        let l = command.len();
                        command[l-1] = &matches[0];
                        let out = command.join(" ");
                        line = format!("{}{}", prompt, out);
                        pos = line.chars().count();
                        print!("{}{}{}", EraseLine, CursorLeft, line);
                    } else {
                        print!("\n");
                        for file in matches {
                            print!("{}   ", file);
                            print!("\n{}", Beep);
                        }
                    }
                }

                KeyCode::Char(c) => {
                    // Handles all other keystrokes basically.
                    let (first, last) = line.split_at(pos);
                    line = format!("{}{}{}", first, c, last);
                    pos = pos + 1;
                    let back = (line.chars().count() - pos) as u16;
                    print!("{}{}{}", EraseLine, CursorLeft, line);
                    if back > 0 { print!("{}", CursorBackward(back)); }
                    let command = line.replace(&prompt, "");
                    let l = history.len();
                    history[l-1] = command;
                }
                _ => {}
            }
            stdout.flush();
        }

        // Move down a line. There. I remember what THIS one does.
        print!("\n");

        // Extract command from the full prompt since I didn't think to keep 'em separate.
        let command = line.replace(&prompt, "");

        // If the command isn't empty, push a new String into the history.
        // The reason for this is the last element in the history is the current command.
        // If the command was successful, we want a new empty string to edit as the command
        // is entered.
        if command != "" {
            history.push(String::new());
            history_pos = history_pos + 1;
        }

        // Finally, we process commands here.
        // If exit, break out of the outer loop and that's all folks!
        if command == "exit" {
            println!("Bye!");
            break;
        // If the command is cd...do cd.
        } else if command.starts_with("cd ") {
            cd(command.split(" ").collect());
        // If it's ls, then do ls.
        } else if command.starts_with("ls ") || command == "ls" {
            ls(command.split(" ").collect());
            stdout.flush();
        // You get the point.
        } else if command == "clear" || command == "cls" {
            clear();
        // If it's none of the above, assume it's a native Windows command.
        } else {
            let mut args: Vec<&str> = command.split(" ").collect();
            let command_sans_args = args[0];
            args.drain(0..1);
            Command::new(command_sans_args).args(args).status().expect("Failed to execute.");
        }
    }
}
