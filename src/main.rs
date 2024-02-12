use std::borrow::Cow::{self, Borrowed, Owned};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;

use chrono::Local;
use clap::Parser;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use rand::Rng;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, USER_AGENT};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use url::form_urlencoded;

use rustyline::completion::FilenameCompleter;
use rustyline::error::ReadlineError;
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::HistoryHinter;
use rustyline::{Completer, Helper, Hinter, Validator};
use rustyline::{CompletionType, Config, Editor};

// const LOADING_CHARS: &str = "/-\\|/-\\|";

#[derive(Helper, Completer, Hinter, Validator)]
struct MyHelper {
    #[rustyline(Completer)]
    completer: FilenameCompleter,
    highlighter: MatchingBracketHighlighter,
    #[rustyline(Hinter)]
    hinter: HistoryHinter,
    colored_prompt: String,
}

impl Highlighter for MyHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        if default {
            Borrowed(&self.colored_prompt)
        } else {
            Borrowed(prompt)
        }
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Owned("\x1b[1m".to_owned() + hint + "\x1b[m")
    }

    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_char(&self, line: &str, pos: usize, _forced: bool) -> bool {
        self.highlighter.highlight_char(line, pos, false)
    }
}

/// Google Gemini CLI
#[derive(Parser, Debug)]
#[command(author = "Seok Won Choi", version, about = "Google Gemini CLI in Rust", long_about = None)]
// #[clap(arg_required_else_help(true))]
struct Args {
    /// __Secure-1PSID
    #[arg(short = 's', long, help = "__Secure-1PSID, usually starts with g.")]
    psid: Option<String>,

    /// __Secure-1PSIDTS
    #[arg(short = 't', long, help = "__Secure-1PSIDTS")]
    psidts: Option<String>,

    /// Markdown
    #[arg(
        short,
        long,
        help = "Path to save the chat as markdown file if available",
        default_value = ""
    )]
    path: String,

    /// Env
    #[arg(
        short,
        long,
        help = "Path to .env file if available",
        default_value = ""
    )]
    env: String,

    /// Multiple answers
    #[arg(short, long, help = "To show multiple answers at once")]
    multi: bool,

    /// Proxy
    #[arg(short = 'x', long, help = "Proxy server", default_value = "")]
    proxy: String,
}

struct Chatbot {
    client: reqwest::Client,
    reqid: u64,
    snlm0e: String,
    conversation_id: String,
    response_id: String,
    choice_id: String,
}

impl Chatbot {
    pub async fn new(_1psid: &str, _1psidts: &str) -> Result<Self, Box<dyn Error>> {
        let cookie = format!("__Secure-1PSID={_1psid}; __Secure-1PSIDTS={_1psidts}");

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36"));
        headers.insert(COOKIE, HeaderValue::from_str(&cookie)?);

        let client_builder = match env::var("GEMINI_PROXY_SERVER") {
            Ok(proxy_server) if !proxy_server.is_empty() => reqwest::Client::builder()
                .default_headers(headers)
                .proxy(reqwest::Proxy::all(&proxy_server)?),
            _ => reqwest::Client::builder().default_headers(headers),
        };

        let client = client_builder.build()?;

        // 1. GET request to https://gemini.google.com/
        let resp = client.get("https://gemini.google.com/").send().await?;
        let body = resp.text().await?;

        // println!("{:#?}", body);

        // 2. Extract SNlM0e value using regex
        let re = Regex::new(r#"SNlM0e":"(.*?)""#).unwrap();
        // println!("Extracting SNlM0e {:#?}", re);
        let snlm0e = re
            .captures(&body)
            .and_then(|caps| caps.get(1).map(|m| m.as_str()))
            .expect("SNlM0e not found. Check your cookies.");

        let reqid: u64 = rand::thread_rng().gen_range(100000..999999);

        Ok(Self {
            client,
            reqid,
            snlm0e: snlm0e.to_owned(),
            conversation_id: String::new(),
            response_id: String::new(),
            choice_id: String::new(),
        })
    }

    async fn ask(
        &mut self,
        message: &str,
        loading_chars: &str,
    ) -> Result<HashMap<String, Value>, Box<dyn Error>> {
        let progress_bar = ProgressBar::new(100);
        // let tick_chars = "⠁⠂⠄⡀⢀⠠⠐⠈ ";
        // let tick_chars = "○○◔◔◑◑◕◕●●◕◕◑◑◔◔ ";
        // let tick_chars = "▁▁▂▂▃▃▄▄▅▅▆▆▇▇██▇▇▆▆▅▅▄▄▃▃▂▂ ";
        // let tick_chars = "-\\|/-\\|/";
        // let tick_chars = "◐◐◓◓◑◑◒◒";
        // let tick_chars = "/-\\|/-\\|";

        progress_bar.set_style(
            ProgressStyle::with_template(
                // "{spinner:.cyan} [{elapsed_precise}] [{wide_bar}] ({percent}%)",
                "[ {spinner:.cyan} {spinner:.red} {spinner:.yellow} {spinner:.green} ] ({percent}% | {elapsed_precise})",
            )
                .unwrap()
                .tick_chars(loading_chars),
        );

        progress_bar.enable_steady_tick(Duration::from_millis(100));
        progress_bar.set_draw_target(ProgressDrawTarget::stdout_with_hz(20)); // redraws at most 20 times per second
        progress_bar.set_position(10u64);

        // 3. Send POST request
        let message_struct = json!([
            [message],
            (),
            [self.conversation_id, self.response_id, self.choice_id],
        ]);
        let form_data = json!([(), message_struct.to_string()]).to_string();

        let body_data = format!(
            "f.req={}&at={}&",
            urlencoding::encode(&form_data),
            urlencoding::encode(&self.snlm0e)
        );

        let encoded: String = form_urlencoded::Serializer::new("https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?".to_string())
            .append_pair("bl", "boq_assistant-bard-web-server_20240201.08_p9")
            .append_pair("_reqid", &self.reqid.to_string())
            .append_pair("rt", "c")
            // .append_pair("hl", "en")
            .finish();

        let mut headers = HeaderMap::new();
        headers.insert(
            "Content-Type",
            HeaderValue::from_static("application/x-www-form-urlencoded;charset=UTF-8"),
        );
        headers.insert(
            "Origin",
            HeaderValue::from_static("https://gemini.google.com"),
        );
        headers.insert(
            "Referer",
            HeaderValue::from_static("https://gemini.google.com/"),
        );

        progress_bar.set_position(rand::thread_rng().gen_range(20..40));

        let post_resp = self
            .client
            .post(encoded)
            .headers(headers)
            .body(body_data)
            .send()
            .await?;

        progress_bar.set_position(rand::thread_rng().gen_range(60..90));

        // Deserialize the JSON string
        let text = post_resp.text().await?;

        let lines: Vec<&str> = text.split('\n').collect();
        let json_str = lines[3];

        let data: Result<Vec<Vec<Value>>, serde_json::Error> = serde_json::from_str(json_str);
        let chat_data = data
            .as_ref()
            .ok()
            .and_then(|inner_data| inner_data.get(0).and_then(|item| item.get(2)));

        let mut results: HashMap<String, Value> = HashMap::new();

        if let Some(chat_data) = chat_data {
            if let Value::String(chat_data_str) = chat_data {
                let json_chat_data: Vec<Value> = serde_json::from_str(chat_data_str)?;

                // println!("{:#?}", json_chat_data);

                results.insert("content".to_string(), json_chat_data[4][0][1][0].clone());
                results.insert("conversation_id".to_string(), json_chat_data[1][0].clone());
                results.insert("response_id".to_string(), json_chat_data[1][1].clone());
                // factualityQueries is now null, so I've removed that line
                results.insert("textQuery".to_string(), json_chat_data[2][0][0].clone());

                let choices: Vec<HashMap<&str, &Value>> = json_chat_data[4]
                    .as_array()
                    .unwrap()
                    .iter()
                    .skip(1) // skip first answer as default
                    .map(|choice| {
                        let mut choice_map = HashMap::new();
                        choice_map.insert("id", &choice[0]);
                        choice_map.insert("content", &choice[1][0]);
                        choice_map
                    })
                    .collect();

                results.insert("choices".to_string(), serde_json::json!(choices));

                // Let's also extract the location information
                if let Some(location) = json_chat_data.get(7) {
                    let mut location_map = HashMap::new();
                    if let Value::String(loc_str) = &location[0] {
                        location_map.insert("address".to_string(), loc_str.clone());
                    }
                    if let Value::String(loc_str) = &location[1] {
                        location_map.insert("place_type".to_string(), loc_str.clone());
                    }
                    results.insert("location".to_string(), serde_json::json!(location_map));
                }

                let conversation_id = results.get("conversation_id").and_then(Value::as_str);
                let response_id = results.get("response_id").and_then(Value::as_str);
                let mut choice_id = results
                    .get("choices")
                    .and_then(Value::as_array)
                    .and_then(|choices| choices.get(0))
                    .and_then(|choice| choice.get("id"))
                    .and_then(Value::as_str)
                    .map(|s| s.to_string());

                // sometimes, there is only one choice.
                // If not found, search for an element that starts with "rc_"
                if choice_id.is_none() {
                    'outer: for item in json_chat_data.iter() {
                        if let Some(array) = item.as_array() {
                            for sub_item in array.iter() {
                                if let Some(s) = sub_item.as_str() {
                                    if s.starts_with("rc_") {
                                        choice_id = Some(s.to_string());
                                        break 'outer;
                                    }
                                } else if let Some(array) = sub_item.as_array() {
                                    for inner_item in array.iter() {
                                        if let Some(s) = inner_item.as_str() {
                                            if s.starts_with("rc_") {
                                                choice_id = Some(s.to_string());
                                                break 'outer;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let (Some(conversation_id), Some(response_id), Some(choice_id)) =
                    (conversation_id, response_id, choice_id)
                {
                    self.conversation_id = conversation_id.to_owned();
                    self.response_id = response_id.to_owned();
                    self.choice_id = choice_id.to_owned();
                    self.reqid += 100000;
                    progress_bar.set_position(100u64);
                } else {
                    eprintln!("Error: couldn't get conversation_id, response_id or choice_id");
                }
            } else {
                eprintln!("Error: chat_data is not a string");
            }
        } else {
            eprintln!("Error: chat_data not found");
        }

        progress_bar.finish_and_clear();

        Ok(results)
    }

    fn reset(&mut self) {
        self.conversation_id.clear();
        self.response_id.clear();
        self.choice_id.clear();
    }
}

async fn append_to_file(file_path: &PathBuf, content: &str) -> Result<(), Box<dyn Error>> {
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(file_path)
        .await?;

    file.write_all(content.as_bytes()).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut loading_chars = "/-\\|/-\\|";

    let args = Args::parse();

    // Load .env file if the path is provided
    if !args.env.is_empty() {
        dotenv::from_path(args.env).ok();
    }

    if !args.proxy.is_empty() {
        env::set_var("GEMINI_PROXY_SERVER", args.proxy.as_str());
    }

    let _1psid = args
        .psid
        .or_else(|| env::var("PSID").ok())
        .or_else(|| {
            // Try loading .env from the current directory
            dotenv::dotenv().ok();
            env::var("PSID").ok()
        })
        .or_else(|| {
            // Try loading .env from the binary's directory
            if let Ok(mut bin_path) = env::current_exe() {
                bin_path.pop(); // Remove the binary name from the path
                bin_path.push(".env"); // Add .env to the path
                dotenv::from_path(bin_path).ok();
                env::var("PSID").ok()
            } else {
                None
            }
        })
        .expect("No session ID provided. Either pass it with -s or provide a .env file");

    let _1psidts = args
        .psidts
        .or_else(|| env::var("PSIDTS").ok())
        .or_else(|| {
            // Try loading .env from the current directory
            dotenv::dotenv().ok();
            env::var("PSIDTS").ok()
        })
        .or_else(|| {
            // Try loading .env from the binary's directory
            if let Ok(mut bin_path) = env::current_exe() {
                bin_path.pop(); // Remove the binary name from the path
                bin_path.push(".env"); // Add .env to the path
                dotenv::from_path(bin_path).ok();
                env::var("PSIDTS").ok()
            } else {
                None
            }
        })
        .unwrap_or_else(|| "".to_string()); // Use an empty string if no value is found
                                            // .expect("No session ID provided. Either pass it with -s or provide a .env file");

    let mut chatbot = Chatbot::new(&_1psid, &_1psidts).await?;

    let mut first_input = true;
    let mut file_path = None;

    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .build();

    let helper = MyHelper {
        completer: FilenameCompleter::new(),
        highlighter: MatchingBracketHighlighter::new(),
        hinter: HistoryHinter {},
        colored_prompt: "".to_owned(),
    };

    let mut rl = Editor::with_config(config)?;
    rl.set_helper(Some(helper));

    let user_prompt = "╭─ You".bright_green();
    let gemini_prompt = "╭─ Gemini".bright_cyan();
    let system_prompt = "╭─ System".bright_red();
    let under_arrow = "╰─>".bright_cyan();
    let under_arrow_red = "╰─>".bright_red();
    let under_arrow_green = "╰─> ";
    let mut last_response: Option<HashMap<String, Value>> = None;

    println!("");
    loop {
        let current_time = Local::now().format("%H:%M:%S").to_string();
        println!("{user_prompt} [{t}]", t = current_time);
        rl.helper_mut().expect("No helper").colored_prompt =
            format!("\x1b[1;32m{p} \x1b[0m", p = under_arrow_green);
        let readline = rl.readline(under_arrow_green);

        match readline {
            Ok(line) => {
                let input = line.trim();
                rl.add_history_entry(input)?;

                if first_input {
                    let file_name = input
                        .chars()
                        .take(10)
                        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                        .collect::<String>()
                        .to_ascii_lowercase()
                        .replace(' ', "_");

                    let file_name = if file_name.is_empty() {
                        "gemini.md".to_string()
                    } else {
                        format!("gemini_{}.md", file_name)
                    };

                    first_input = false;
                    file_path = if !args.path.trim().is_empty() {
                        let mut path = PathBuf::from(&args.path);
                        if !args.path.ends_with('/') {
                            path.push("/");
                        }
                        Some(path.join(&file_name))
                    } else {
                        None
                    };
                }

                if input == "!exit" {
                    break;
                } else if input == "!reset" {
                    chatbot.reset();
                } else if input == "!settings" {
                    println!("\n{}", system_prompt);
                    println!("{under_arrow_red} Please select a progress bar style: ");

                    let tick_chars = vec![
                        "⠁⠂⠄⡀⢀⠠⠐⠈",
                        "○○◔◔◑◑◕◕●●◕◕◑◑◔◔",
                        "▁▁▂▂▃▃▄▄▅▅▆▆▇▇██▇▇▆▆▅▅▄▄▃▃▂▂",
                        "-\\|/-\\|/",
                        "◐◐◓◓◑◑◒◒",
                        "/-\\|/-\\|",
                    ];

                    // Display the tick characters
                    for (i, chars) in tick_chars.iter().enumerate() {
                        println!("{}. {}", i + 1, chars);
                    }

                    let mut style_choice = String::new();
                    std::io::stdin()
                        .read_line(&mut style_choice)
                        .expect("Failed to read line");
                    let style_choice: usize = style_choice
                        .trim()
                        .parse()
                        .expect("Please input a valid number");

                    if style_choice > tick_chars.len() || style_choice < 1 {
                        println!("Invalid selection.");
                    } else {
                        let selected_style = &tick_chars[style_choice - 1];
                        // ... apply this style to your actual progress bar
                        println!("Selected style: {}", selected_style);
                        loading_chars = selected_style;
                    }
                } else if input == "!show" {
                    if let Some(ref res) = last_response {
                        let current_time = Local::now().format("%H:%M:%S").to_string();

                        println!("\n{gemini_prompt} [{current_time}]");
                        let array = res.get("choices").unwrap().as_array().unwrap();

                        for (i, object) in array.iter().enumerate() {
                            if let Some(content) = object["content"].as_str() {
                                println!("\r{} {}. {}\n", under_arrow, i + 1, content);
                            }
                        }
                    }
                } else {
                    if let Some(file_path) = &file_path {
                        append_to_file(file_path, &format!("**You**: {}\n\n", input)).await?;
                    }
                    let current_time = Local::now().format("%H:%M:%S").to_string();
                    println!("\n{gemini_prompt} [{current_time}]");

                    let response = chatbot.ask(input, loading_chars).await?;
                    // print!("{} thinking...", under_arrow);
                    // stdout().flush().unwrap(); // Flush the output

                    print!("\r");

                    let response_content = response.get("content").unwrap().as_str().unwrap();

                    if args.multi {
                        let array = response.get("choices").unwrap().as_array().unwrap();

                        for (i, object) in array.iter().enumerate() {
                            if let Some(content_array) = object["content"].as_array() {
                                for string in content_array {
                                    if let Some(s) = string.as_str() {
                                        println!("\r{} {}. {}\n", under_arrow, i + 1, s);
                                    }
                                }
                            }
                        }
                    } else {
                        // Use \r to move the cursor to the beginning of the line and print the response
                        println!("\r{} {}\n", under_arrow, response_content); // Print the second line
                    }

                    if let Some(file_path) = &file_path {
                        append_to_file(file_path, &format!("**Gemini**: {}\n\n", response_content))
                            .await?;
                    }

                    last_response = Some(response);
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                println!("\nInterrupt signal detected, exiting...");
                break;
            }
            Err(_) => {
                continue;
            }
        }
    }

    Ok(())
}
