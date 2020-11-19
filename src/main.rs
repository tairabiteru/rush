use std::process::{Command};
use std::io;
use std::io::{Write, ErrorKind};
use std::env;
use std::os::windows::prelude::*;
use colored::*;
use std::path::{Path, Component};
use term_size;
use ansi_escapes::*;
use crossterm::event::{self, Event, KeyCode, KeyEvent};


// Obtain CWD as String without \\?\
fn cwd_as_string() -> String {
    let abs = env::current_dir().unwrap().canonicalize().unwrap();
    let cwd = abs.into_os_string().into_string().unwrap();
    return cwd.replace("\\\\?\\", "");
}

// Obtain %HOMEPATH%
fn home_as_string() -> String {
    let home = env::var("HOMEPATH").unwrap();
    return home.replace("\\\\?\\", "");
}


// Turn a path into a linux path
fn linuxize_path(directory: String) -> String {
    let home = home_as_string().replace("\\", "/");
    let mut output = directory.clone();
    output = output.replace(&home, "~");
    output = output.replace("\\", "/");
    return output;
}


// Match files in a directory
fn match_files(directory: String, hint: String) -> Vec<String> {
    let mut matches: Vec<String> = Vec::new();
    let dir = Path::new(&directory);
    let files = match dir.read_dir() {
        Ok(files) => files,
        Err(error) => panic!("Could not obtain directory listing: {:?}", error),
    };

    for file in files {
        if let Ok(file) = file {
            let filename = file.file_name().into_string().unwrap();
            if hint == "" {
                matches.push(filename.clone());
            } else if filename.starts_with(&hint) {
                matches.push(filename.clone());
            }
        }
    }
    return matches;
}


// Emulate a """simple""" console.
// (lol jk, there's no such thing)
struct Console {
    command: String,
    cursor_pos: usize,
    stdout: io::Stdout,
    history: Vec<String>,
    history_pos: usize,
}


impl Console {

    // Initialize fields.
    fn new() -> Self {
        Console {
            command: String::new(),
            cursor_pos: 0,
            stdout: io::stdout(),
            history: vec![String::new()],
            history_pos: 0,
        }
    }


    // Clear out entire screen
    fn clear(&self) {
        Command::new("cmd").args(&["/C", "cls"]).status();
    }


    // DING A LING
    fn bell(&mut self) {
        print!("{}", Beep);
        self.stdout.flush();
    }


    // Obtain terminal width
    fn term_width(&self) -> usize {
        let mut term_width = 100;
        if let Some((w, _h)) = term_size::dimensions() {
            term_width = w;
        } else {
            println!("Unable to obtain terminal size. Assuming 100.");
        }
        return term_width;
    }


    // Obtain user@host:dir$
    fn get_prompt(&self) -> String {
        let user = env::var("USERNAME").unwrap().to_lowercase();
        let host = env::var("COMPUTERNAME").unwrap().to_lowercase();
        let prompt = format!("{}@{}", user, host).bright_green().bold().to_string();

        let dir = cwd_as_string();
        let home = home_as_string();
        let dir = dir.replace(&home, "~");
        let dir = dir.replace("\\", "/");

        return format!("{}:{}{} ", prompt, dir.bright_blue().bold(), "$".normal().clear());
    }


    // Called to render the contents of self.command to the console
    fn render(&mut self) {
        let back = (self.command.chars().count() - self.cursor_pos) as u16;
        print!("{}{}", EraseLine, CursorLeft);
        print!("{}{}", self.get_prompt(), self.command);
        if back > 0 {
            print!("{}", CursorBackward(back));
        }
        self.stdout.flush();
    }


    // Handle backspace keystroke
    fn handle_backspace(&mut self) {
        if self.cursor_pos <= 0 {
            self.bell();
        } else {
            let (s1, s2) = self.command.split_at(self.cursor_pos);
            let mut s1 = String::from(s1);
            s1.pop();
            self.command = format!("{}{}", s1, s2);
            self.cursor_pos -= 1;
            let hislen = self.history.len();
            self.history[hislen-1] = self.command.clone();
        }
        self.render();
    }


    // Handle left arrow keystroke
    fn handle_left(&mut self) {
        if self.cursor_pos <= 0 {
            self.bell();
        } else {
            print!("{}", CursorBackward(1));
            self.cursor_pos -= 1;
            self.stdout.flush();
        }
    }


    // Handle right arrow keystroke
    fn handle_right(&mut self) {
        if self.cursor_pos >= self.command.chars().count() {
            self.bell();
        } else {
            print!("{}", CursorForward(1));
            self.cursor_pos += 1;
            self.stdout.flush();
        }
    }


    // Handle up arrow keystroke
    fn handle_up(&mut self) {
        if self.history_pos > 0 {
            self.history_pos -= 1;
            self.command = self.history[self.history_pos].clone();
            self.cursor_pos = self.command.chars().count();
            self.render();
        } else {
            self.bell();
        }
    }


    // Handle down arrow keystroke
    fn handle_down(&mut self) {
        if self.history.len() <= 1 { return self.bell(); }
        if self.history_pos < (self.history.len() - 1) {
            self.history_pos += 1;
            self.command = self.history[self.history_pos].clone();
            self.cursor_pos = self.command.chars().count();
            self.render();
        } else {
            self.bell()
        }
    }

    // Handle tab keystroke
    // Please don't look at this. It makes me want to cry.
    fn handle_tab(&mut self) {

        // I TOLD YOU NOT TO LOOK AT IT.

        // Ugh, fine. So here we ding the bell if they've either entered nothing
        // Or if the command contains no space. If it contains no space, then they
        // are still typing the command.
        if self.command == "" || !self.command.contains(" ") { return self.bell(); }

        // We have to get the first index by the first space character.
        // This is because paths can contain spaces too, so we can't split by spaces.
        let fsi = self.command.chars().position(|c| c == ' ').unwrap();
        let (command, a) = self.command.split_at(fsi);
        let args = a.trim_start();
        let dir;

        // Please note: the rest of this was written by an idiot.

        // Here, we replace ~ with the full home path.
        if args.starts_with("~") {
            let mut chars = args.chars();
            chars.next();
            dir = format!("{}{}", home_as_string(), chars.as_str());
        } else {
            dir = args.to_string();
        }

        // In short, this massive clusterfuck is responsible for iterating over path components
        // and ensuring each step along the path is valid. If it's not, that is the value
        // we want to try to predict.
        let components: Vec<Component> = Path::new(&dir).components().collect();
        let mut last_good = String::new();
        let mut last_component = String::new();
        let mut valid = true;

        for component in components {
            last_component = component.as_os_str().to_os_string().into_string().unwrap();
            last_component = last_component.replace("\\", "/");
            match component {
                Component::RootDir => last_good = String::from("/"),
                _ => {
                    let test;
                    if last_good.ends_with("/"){
                        test = format!("{}{}", last_good, last_component);
                    } else {
                        test = format!("{}/{}", last_good, last_component);
                    }

                    if Path::new(&test).exists() {
                        last_good = test.clone();
                    } else {
                        valid = false;
                        break;
                    }
                }
            }
        }
        let matches;

        // If the whole path is valid, then the predictor needs to
        // match all files in the directory, so we need to pass ""
        if valid {
            matches = match_files(last_good.clone(), String::new());

        // Otherwise, try matching by the last component.
        } else {
            matches = match_files(last_good.clone(), last_component);
        }

        // If 0 matches, BEEP AT THEM.
        if matches.len() == 0 {
            self.bell();
        // For now, if they get more than one match, we beep. But eventually ls will supplant this.
        } else if matches.len() != 1 {
            self.bell();
        // If there's exactly one match, we've got work to do.
        } else {
            let mut autocompleted;
            // If the dir already ends with /, we don't need to append a /.
            // Also if there was no last known good one, it means we're at the root.
            // So also don't append /.
            if last_good.ends_with("/") || last_good == "" {
                autocompleted = format!("{}{}", last_good, matches[0]);
            } else {
                autocompleted = format!("{}/{}", last_good, matches[0]);
            }

            // Now check to see if the path is a dir. If it is, and if it doesn't already,
            // end it with a /.
            let path = Path::new(&autocompleted);
            if path.exists() {
                if path.metadata().unwrap().is_dir() && !autocompleted.ends_with("/") {
                    autocompleted = format!("{}/", autocompleted);
                }
            }

            // Finally, this is over.
            autocompleted = linuxize_path(autocompleted);
            self.command = format!("{} {}", command, autocompleted);
            self.cursor_pos = self.command.chars().count();
            self.render();
        }
    }
    // Suffering has ended. Please resume normal activity.


    // Handle all other char input
    fn handle_char_input(&mut self, c: char) {
        let (s1, s2) = self.command.split_at(self.cursor_pos);
        self.command = format!("{}{}{}", s1, c, s2);
        self.cursor_pos += 1;
        let hislen = self.history.len();
        self.history[hislen-1] = self.command.clone();
        self.render();
    }


    // Push new string to history, thereby effectively committing the last one.
    // This is a separate function because we don't always necessarily want to
    // do this, and sometimes the deciding factor will be outside of the Console
    // object. (I.E. an invalid command)
    fn history_push(&mut self) {
        self.history.push(String::new());
        self.history_pos += 1;
    }


    // Where it all comes together!
    // Keystroke events are read in here.
    fn await_command(&mut self) -> String {
        print!("{}", self.get_prompt());
        self.stdout.flush();
        self.command = String::new();
        self.cursor_pos = 0;
        while let Event::Key(KeyEvent{code, ..}) = event::read().unwrap() {
            match code {

                KeyCode::Backspace => {
                    self.handle_backspace();
                }

                KeyCode:: Tab=> {
                    self.handle_tab();
                }

                KeyCode::Up => {
                    self.handle_up();
                }

                KeyCode::Down => {
                    self.handle_down();
                }

                KeyCode::Left => {
                    self.handle_left();
                }

                KeyCode::Right => {
                    self.handle_right();
                }

                KeyCode::Enter => {
                    break;
                }

                KeyCode::Char(c) => {
                    self.handle_char_input(c);
                }

                _ => {}

            }
        }
        print!("\n");
        return self.command.clone();
    }
}


// ls directories
fn ls(directory: String, console: &Console) {
    let width = console.term_width();
    let dir = directory.replace("~", &home_as_string());
    let path = Path::new(&dir);

    let files = match path.read_dir() {
        Ok(files) => files,
        Err(error) => return println!("Could not read directory: {:?}", error),
    };

    let mut charcount = 0;
    let mut perfect = true;

    // This doesn't need to be mut, but it is because it eventually will need to be.
    let mut list_hidden = false;

    for file in files {
        if let Ok(file) = file {
            let filename = file.file_name().into_string().unwrap();
            let meta = file.metadata().unwrap();
            let attr = meta.file_attributes();
            let mut output = String::new();

            if (charcount + &filename.chars().count()) > width {
                output = format!("\n\n{}   ", &filename);
                charcount = filename.chars().count() + 3;
            } else if (charcount + &filename.chars().count()) == width {
                output = format!("{}\n\n", &filename);
                charcount = 0;
            } else {
                output = format!("{}   ", &filename);
                charcount += filename.chars().count() + 3;
            }

            if (attr == 2 || filename.starts_with(".")) && !list_hidden {
                continue;
            } else if meta.is_dir() {
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


// Change directories
fn cd(directory: String) {
    let dir = directory.replace("~", &home_as_string());
    let path = Path::new(&dir);

    let result = env::set_current_dir(&path);
    match result {
        Ok(output) => output,
        Err(error) => match error.kind() {
            ErrorKind::NotFound => {
                println!("Could not cd into \"{}\": Directory not found.", path.display());
            },
            ErrorKind::PermissionDenied => {
                println!("Could not cd into \"{}\": Access is denied.", path.display());
            }
            other => {
                println!("Could not cd into \"{}\": {:?}", path.display(), other);
            }
        }
    };
}


// main
fn main() {
    // OOP (kinda anyway)
    let mut console = Console::new();
    console.clear();

    loop {
        // This amounts to a really fancy read_line()
        let command = console.await_command();

        // Separate command from args
        let args;
        if command.contains(" ") {
            let fsi = command.chars().position(|c| c == ' ').unwrap();
            let (_command, a) = command.split_at(fsi);
            args = a.trim_start();
        } else {
            args = "";
        }

        // Process commands
        if command.starts_with("ls") {
            if command == "ls" {
                ls(cwd_as_string(), &console);
            } else {
                ls(args.to_string(), &console);
            }
        } else if command == "clear" || command == "cls" {
            console.clear();
        } else if command.starts_with("cd ") {
            cd(args.to_string());
        } else if command == "exit" {
            break;
        } else {
            let mut args: Vec<&str> = command.split(" ").collect();
            let command_sans_args = args[0];
            args.drain(0..1);
            Command::new(command_sans_args).args(args).status();
        }
        console.history_push();
    }
    println!("Bye!");
}
