use futures_util::io::AsyncWriteExt;

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
use url::form_urlencoded;

use rustyline_async::{Readline, ReadlineEvent, SharedWriter};

// const LOADING_CHARS: &str = "/-\\|/-\\|";

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
        // println!("{}", cookie);

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"));
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

        // 2. Check if the body contains the word "CAPTCHA"
        if body.contains("CAPTCHA") {
            panic!(
                "ERROR: Google detected it as a malicious action. The block will expire shortly after those requests stop. Try again later."
            );
        }

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
        writer: &mut SharedWriter,
    ) -> Result<HashMap<String, Value>, Box<dyn Error>> {
        let progress_bar = ProgressBar::new(100);
        // let tick_chars = "⠁⠂⠄⡀⢀⠠⠐⠈ ";
        // let tick_chars = "○○◔◔◑◑◕◕●●◕◕◑◑◔◔ ";
        // let tick_chars = "▁▁▂▂▃▃▄▄▅▅▆▆▇▇██▇▇▆▆▅▅▄▄▃▃▂▂ ";
        // let tick_chars = "-\\|/-\\|/";
        // let tick_chars = "◐◐◓◓◑◑◒◒";
        // let tick_chars = "/-\\|/-\\|";
        writer.write_all(b"\r\x1b[2K").await?; // Clear the line after the progress bar

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
            .append_pair("bl", "boq_assistant-bard-web-server_20240717.08_p5")
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

    tokio::io::AsyncWriteExt::write_all(&mut file, content.as_bytes()).await?;
    Ok(())
}

// Function to encapsulate the repeated logic
fn get_env_var_or_dotenv(var_name: &str) -> Option<String> {
    env::var(var_name)
        .ok()
        .or_else(|| {
            dotenv::dotenv().ok();
            env::var(var_name).ok()
        })
        .or_else(|| {
            if let Ok(mut bin_path) = env::current_exe() {
                bin_path.pop(); // Remove the binary name from the path
                bin_path.push(".env"); // Add .env to the path
                dotenv::from_path(bin_path).ok();
                env::var(var_name).ok()
            } else {
                None
            }
        })
}

fn strip_ansi_codes(s: &str) -> String {
    let re = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    re.replace_all(s, "").to_string()
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

    let _1psid = get_env_var_or_dotenv("PSID")
        .expect("No session ID provided. Either pass it with -s or provide a .env file");

    let _1psidts = get_env_var_or_dotenv("PSIDTS").unwrap_or_else(|| "".to_string());

    // Attempt to get the path from command-line arguments or environment variable
    let history_path = if !args.path.trim().is_empty() {
        args.path.clone()
    } else {
        get_env_var_or_dotenv("GEMINI_HISTORY").unwrap_or_else(|| "".to_string())
    };

    let mut chatbot = Chatbot::new(&_1psid, &_1psidts).await?;

    let mut first_input = true;
    let mut file_path = None;

    let user_prompt = "╭─ You".bright_green().to_string();
    let gemini_prompt = "╭─ Gemini".bright_cyan().to_string();
    let system_prompt = "╭─ System".bright_red().to_string();
    let under_arrow = "╰─>".bright_cyan().to_string();
    let under_arrow_red = "╰─>".bright_red().to_string();
    let under_arrow_green = ">-"; // TODO: won't color it as it harms cursor position

    let mut last_response: Option<HashMap<String, Value>> = None;
    let (mut readline, mut writer) = Readline::new(format!("{under_arrow_green} "))?;
    // the input line does not remain on screen after Enter
    readline.should_print_line_on(true, true);

    writer.write_all(b"\n").await?;
    loop {
        let current_time = Local::now().format("%H:%M:%S").to_string();
        writer
            .write_all(format!("{user_prompt} [{t}]\n", t = current_time).as_bytes())
            .await?;

        match readline.readline().await {
            Ok(ReadlineEvent::Line(line)) => {
                let input = line.trim().to_string();
                readline.add_history_entry(input.clone());

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
                    file_path = if !history_path.trim().is_empty() {
                        let mut path = PathBuf::from(&history_path);
                        if !history_path.ends_with('/') {
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
                    writer
                        .write_all(format!("\n{system_prompt}\n").as_bytes())
                        .await?;
                    writer
                        .write_all(
                            format!("{under_arrow_red} Please select a progress bar style: \n")
                                .as_bytes(),
                        )
                        .await?;

                    let tick_chars = vec![
                        "⠁⠂⠄⡀⢀⠠⠐⠈",
                        "○○◔◔◑◑◕◕●●◕◕◑◑◔◔",
                        "▁▁▂▂▃▃▄▄▅▅▆▆▇▇██▇▇▆▆▅▅▄▄▃▃▂▂",
                        "-\\|/-\\|/",
                        "◐◐◓◓◑◑◒◒",
                        "/-\\|/-\\|",
                    ];

                    for (i, chars) in tick_chars.iter().enumerate() {
                        writer
                            .write_all(format!("{}. {}\n", i + 1, chars).as_bytes())
                            .await?;
                    }

                    let style_choice = match readline.readline().await? {
                        ReadlineEvent::Line(line) => line,
                        _ => {
                            writer
                                .write_all(b"Invalid input. Exiting settings.\n")
                                .await?;
                            continue;
                        }
                    };

                    let style_choice: usize = style_choice.trim().parse().unwrap_or(0);

                    if style_choice > tick_chars.len() || style_choice < 1 {
                        writer.write_all(b"Invalid selection.\n").await?;
                    } else {
                        let selected_style = &tick_chars[style_choice - 1];
                        writer
                            .write_all(format!("Selected style: {}\n", selected_style).as_bytes())
                            .await?;
                        loading_chars = selected_style;
                    }
                } else if input == "!show" {
                    if let Some(ref res) = last_response {
                        let current_time = Local::now().format("%H:%M:%S").to_string();

                        writer
                            .write_all(format!("\n\n{gemini_prompt} [{current_time}]\n").as_bytes())
                            .await?;
                        let array = res.get("choices").unwrap().as_array().unwrap();

                        for (i, object) in array.iter().enumerate() {
                            if let Some(content) = object["content"].as_str() {
                                writer
                                    .write_all(
                                        format!("{} {}. {}\n", under_arrow, i + 1, content)
                                            .as_bytes(),
                                    )
                                    .await?;
                            }
                        }
                    }
                } else {
                    if let Some(file_path) = &file_path {
                        append_to_file(file_path, &format!("**You**: {}\n\n", input)).await?;
                    }
                    let current_time = Local::now().format("%H:%M:%S").to_string();

                    readline.flush()?;
                    writer.write_all(b"\r").await?; // Clear the line before the progress bar

                    let response = chatbot.ask(&input, loading_chars, &mut writer).await?;

                    let response_content = response.get("content").unwrap().as_str().unwrap();

                    writer
                        .write_all(format!("\n\n{gemini_prompt} [{current_time}]\n").as_bytes())
                        .await?;

                    if args.multi {
                        let array = response.get("choices").unwrap().as_array().unwrap();

                        for (i, object) in array.iter().enumerate() {
                            if let Some(content_array) = object["content"].as_array() {
                                for string in content_array {
                                    if let Some(s) = string.as_str() {
                                        writer
                                            .write_all(
                                                format!("{} {}. {}\n", under_arrow, i + 1, s)
                                                    .as_bytes(),
                                            )
                                            .await?;
                                    }
                                }
                            }
                        }
                    } else {
                        writer
                            .write_all(format!("{} {}\n", under_arrow, response_content).as_bytes())
                            .await?;
                    }

                    if let Some(file_path) = &file_path {
                        append_to_file(file_path, &format!("**Gemini**: {}\n\n", response_content))
                            .await?;
                    }

                    last_response = Some(response);
                }
            }
            Ok(ReadlineEvent::Eof) => {
                writer.write_all(b"\nEOF detected, exiting...\n").await?;
                break;
            }
            Ok(ReadlineEvent::Interrupted) => {
                writer
                    .write_all(b"\nInterrupt signal detected, exiting...\n")
                    .await?;
                break;
            }
            Err(_) => {
                continue;
            }
        }
    }

    readline.flush()?;
    Ok(())
}
