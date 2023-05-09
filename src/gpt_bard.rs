use rand::Rng;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, USER_AGENT};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::error::Error;
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use url::form_urlencoded;

use clap::Parser;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_openai::{
    types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role},
    Client as OpenAIClient,
};

/// Google Bard CLI
#[derive(Parser, Debug)]
#[command(author = "Seok Won Choi", version, about = "Google Bard CLI in Rust", long_about = None)]
struct Args {
    /// __Secure-1PSID
    #[arg(short, long, help = "About 71 length long, including '.' in the end.")]
    session: Option<String>,

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
    pub async fn new(session_id: &str) -> Result<Self, Box<dyn Error>> {
        let cookie = format!("__Secure-1PSID={session_id}");

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36"));
        headers.insert(COOKIE, HeaderValue::from_str(&cookie)?);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        // 1. GET request to https://bard.google.com/
        let resp = client.get("https://bard.google.com/").send().await?;
        let body = resp.text().await?;

        // 2. Extract SNlM0e value using regex
        let re = Regex::new(r#"SNlM0e":"(.*?)""#).unwrap();
        let snlm0e = re
            .captures(&body)
            .and_then(|caps| caps.get(1).map(|m| m.as_str()))
            .expect("SNlM0e not found");

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

    async fn ask(&mut self, message: &str) -> Result<HashMap<String, Value>, Box<dyn Error>> {
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

        let encoded: String = form_urlencoded::Serializer::new("https://bard.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?".to_string())
            .append_pair("bl", "boq_assistant-bard-web-server_20230507.20_p2")
            .append_pair("_reqid", &self.reqid.to_string())
            .append_pair("rt", "c")
            .finish();

        let mut headers = HeaderMap::new();
        headers.insert(
            "Content-Type",
            HeaderValue::from_static("application/x-www-form-urlencoded;charset=UTF-8"),
        );
        headers.insert(
            "Origin",
            HeaderValue::from_static("https://bard.google.com"),
        );
        headers.insert(
            "Referer",
            HeaderValue::from_static("https://bard.google.com/"),
        );

        let post_resp = self
            .client
            .post(encoded)
            .headers(headers)
            .body(body_data)
            .send()
            .await?;

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

                results.insert("content".to_string(), json_chat_data[0][0].clone());
                results.insert("conversation_id".to_string(), json_chat_data[1][0].clone());
                results.insert("response_id".to_string(), json_chat_data[1][1].clone());
                results.insert("factualityQueries".to_string(), json_chat_data[3].clone());
                results.insert("textQuery".to_string(), json_chat_data[2][0].clone());

                let choices: Vec<HashMap<&str, &Value>> = json_chat_data[4]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|choice| {
                        let mut choice_map = HashMap::new();
                        choice_map.insert("id", &choice[0]);
                        choice_map.insert("content", &choice[1]);
                        choice_map
                    })
                    .collect();

                results.insert("choices".to_string(), serde_json::json!(choices));

                let conversation_id = results.get("conversation_id").and_then(Value::as_str);
                let response_id = results.get("response_id").and_then(Value::as_str);
                let choice_id = results
                    .get("choices")
                    .and_then(Value::as_array)
                    .and_then(|choices| choices.get(0))
                    .and_then(|choice| choice.get("id"))
                    .and_then(Value::as_str);

                if let (Some(conversation_id), Some(response_id), Some(choice_id)) =
                    (conversation_id, response_id, choice_id)
                {
                    self.conversation_id = conversation_id.to_owned();
                    self.response_id = response_id.to_owned();
                    self.choice_id = choice_id.to_owned();
                    self.reqid += 100000;
                } else {
                    eprintln!("Error: couldn't get conversation_id, response_id or choice_id");
                }
            } else {
                eprintln!("Error: chat_data is not a string");
            }
        } else {
            eprintln!("Error: chat_data not found");
        }

        Ok(results)
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
    let args = Args::parse();

    // Load .env file if the path is provided
    if !args.env.is_empty() {
        dotenv::from_path(args.env).ok();
    }

    let session_id = args
        .session
        .or_else(|| std::env::var("SESSION_ID").ok())
        .expect("No session ID provided. Either pass it with -s or provide a .env file");

    let openai_key = std::env::var("API_KEY").expect("API_KEY not set");
    let openai_client = OpenAIClient::new().with_api_key(openai_key.clone());

    let mut chatbot = Chatbot::new(&session_id).await?;

    // Create a shared exit flag using Arc<AtomicBool>
    let exit_flag = Arc::new(AtomicBool::new(false));
    let exit_flag_signal_handler = exit_flag.clone();
    let (input_tx, input_rx) = mpsc::channel();

    // Spawn a separate task to listen for the ctrl+c signal
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        exit_flag_signal_handler.store(true, Ordering::SeqCst);
    });

    // Spawn a separate thread for reading user input
    // likely because stdin internally uses spawn_blocking, so it is impossible to interrupt the read.
    std::thread::spawn(move || loop {
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_ok() {
            if input_tx.send(input).is_err() {
                break;
            }
        } else {
            break;
        }
    });

    let file_path = if !args.path.is_empty() {
        let mut save_path = PathBuf::from(&args.path);
        save_path.push("ai.md");
        Some(save_path)
    } else {
        None
    };

    // Initial message to start the conversation
    let mut message =
        String::from("You will have a conversation with me, but my message is from you. Even if there is a loop, you will respond as if there were a new thing said.");

    println!("Starting a conversation with '{message}'");

    'outer: loop {
        // Bard replies
        print!("\rBard: thinking...");
        stdout().flush().unwrap(); // Flush the output

        let response = chatbot.ask(&message).await?;
        let bard_response = response.get("content").unwrap().as_str().unwrap();

        // Use \r to move the cursor to the beginning of the line and print the response
        println!("\r< Bard: {}\n", bard_response);

        if let Some(file_path) = &file_path {
            append_to_file(file_path, &format!("**Bard**: {}\n\n", bard_response)).await?;
        }

        // GPT-3.5 replies
        print!("\rGPT-3.5: thinking...");
        stdout().flush().unwrap(); // Flush the output

        // GPT-3.5 talks
        let gpt3_response =
            talk_gpt(bard_response.clone(), &openai_client, "gpt-3.5-turbo").await?;
        println!("\r< GPT-3.5: {}", gpt3_response);

        if let Some(file_path) = &file_path {
            append_to_file(file_path, &format!("**GPT-3.5**: {}\n\n", gpt3_response)).await?;
        }

        // GPT-4 replies
        print!("\rGPT-4: thinking...");
        stdout().flush().unwrap(); // Flush the output

        // GPT-4 talks
        let gpt4_response = talk_gpt(gpt3_response.clone(), &openai_client, "gpt-4").await?;
        println!("\r> GPT-4: {}", gpt4_response);

        if let Some(file_path) = &file_path {
            append_to_file(file_path, &format!("**GPT-4**: {}\n\n", gpt4_response)).await?;
        }

        message = gpt4_response.to_owned(); // Set the new message to Bard's response

        // Wait for user input (press Enter) to continue
        print!("Press Enter to continue...");
        stdout().flush().unwrap(); // Flush the output

        // catch ctrl+c
        loop {
            // Receive input from the input thread with a timeout
            match input_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(_) => {
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {
                    if exit_flag.load(Ordering::SeqCst) {
                        println!("\nCtrl+C detected, exiting...");
                        break 'outer;
                    }
                }
                Err(_) => break,
            }
        }
    }

    Ok(())
}

async fn talk_gpt(
    interest: String,
    openai_client: &OpenAIClient,
    model: &str,
) -> Result<String, Box<dyn Error>> {
    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(512u16)
        .model(model)
        .messages([
            ChatCompletionRequestMessageArgs::default()
                .role(Role::System)
                .content("You are a helpful assistant.")
                .build()?,
            ChatCompletionRequestMessageArgs::default()
                .role(Role::User)
                .content(interest)
                .build()?,
        ])
        .build()?;

    let response = openai_client.chat().create(request).await?;
    Ok(response.choices[0].message.content.clone())
}

/*
/// group talk

'outer: loop {
    // Send the message to all AI models and receive their responses
    let gpt3_5_future = talk_gpt(message.clone(), &openai_client, "gpt-3.5-turbo");
    let gpt4_future = talk_gpt(message.clone(), &openai_client, "gpt-4");
    let bard_future = chatbot.ask(&message);

    let (gpt3_5_response, gpt4_response, bard_response) =
        tokio::join!(gpt3_5_future, gpt4_future, bard_future);

    let gpt3_5_response = gpt3_5_response?;
    let gpt4_response = gpt4_response?;

    let temp_bard_response = bard_response.expect("Failed to get response from Bard");
    let bard_response = temp_bard_response.get("content").unwrap().as_str().unwrap();

    // Print and store the responses
    println!("< GPT-3.5: {}", gpt3_5_response);
    println!("> GPT-4: {}", gpt4_response);
    println!("< Bard: {}\n", bard_response);

    if let Some(file_path) = &file_path {
        append_to_file(file_path, &format!("**GPT-3.5**: {}\n\n", gpt3_5_response)).await?;
        append_to_file(file_path, &format!("**GPT-4**: {}\n\n", gpt4_response)).await?;
        append_to_file(file_path, &format!("**Bard**: {}\n\n", bard_response)).await?;
    }

    // Set the new message based on user input or a randomly selected response from one of the AI models
    print!("Type a new message or press Enter to continue...");
    stdout().flush().unwrap(); // Flush the output

    // catch ctrl+c and user input
    loop {
        // Receive input from the input thread with a timeout
        match input_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(input) => {
                if input.trim().is_empty() {
                    // Randomly select a response from one of the AI models as the new message
                    let responses = vec![&gpt3_5_response, &gpt4_response, bard_response];
                    message = responses
                        .choose(&mut rand::thread_rng())
                        .unwrap()
                        .to_string();
                } else {
                    message = input.trim().to_string();
                }
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                if exit_flag.load(Ordering::SeqCst) {
                    println!("\nCtrl+C detected, exiting...");
                    break 'outer;
                }
            }
            Err(_) => break,
        }
    }
*/
