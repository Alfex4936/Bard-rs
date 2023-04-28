use rand::Rng;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, USER_AGENT};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::error::Error;
use std::io::{stdout, Write};
use url::form_urlencoded;

use clap::Parser;

/// Google Bard CLI
#[derive(Parser, Debug)]
#[command(author = "Seok Won Choi", version, about = "Google Bard CLI in Rust", long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long, help = "About 71 length long, including '.' in the end.")]
    session: String,
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
            .append_pair("bl", "boq_assistant-bard-web-server_20230426.11_p0")
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
                let json_chat_data: Vec<Value> = serde_json::from_str(&chat_data_str)?;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let session_id = args.session;
    let mut chatbot = Chatbot::new(&session_id).await?;

    let mut exit = false;
    while !exit {
        print!("You: ");
        stdout().flush().unwrap(); // Flush the output

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        let input = input.trim();

        if input == "!exit" {
            exit = true;
        } else if input == "!reset" {
            chatbot.conversation_id.clear();
            chatbot.response_id.clear();
            chatbot.choice_id.clear();
        } else {
            // println!("{}", input); // Print the user's input
            print!("Bard: thinking...");
            stdout().flush().unwrap(); // Flush the output

            let response = chatbot.ask(input).await?;

            // Use \r to move the cursor to the beginning of the line and print the response
            println!(
                "\rBard: {}",
                response.get("content").unwrap().as_str().unwrap()
            );
        }
    }

    Ok(())
}