# <img src="https://user-images.githubusercontent.com/2356749/235068474-5daddf05-54d6-4391-ae97-1a944aebdec6.png" style="height: 32px"> Google Gemini CLI

A simple command line interface for interacting with Google Gemini (Previously `Bard`), written in Rust.

This CLI allows you to save chat history as a Markdown file at a specified absolute or relative path in realtime

and handles graceful exit with Ctrl+C.

![image](https://github.com/Alfex4936/Bard-rs/assets/2356749/168fbea0-fb85-4cc3-ae16-c6936b17b2c1)

---

![output](https://github.com/Alfex4936/Bard-rs/assets/2356749/1a81dc59-2be0-4812-afcc-537c29f71919)

## Prerequisites

You need to have Rust and Cargo installed on your system. If you don't have them, you can install them from the [official Rust website](https://www.rust-lang.org/tools/install).

## Installation

Install from cargo. Add `-f` at the end to force update. (`cargo install bard-rs -f`)

```bash
cargo install bard-rs
```

or

1. Clone the repository to your local machine:

   ```
   git clone https://github.com/Alfex4936/Bard-rs
   ```

2. Change the working directory:

   ```
   cd Bard-rs
   ```

3. Build the project:

   ```
   cargo build --release
   ```

The executable binary file will be located in the `target/release` folder.

## Usage

Before using the Google Gemini CLI, you need to obtain your session cookie. To get the session cookie, follow these steps:

1. Go to [Google Gemini](https://gemini.google.com/) in Chrome.
2. Open Chrome Developer Tools (F12 or `Ctrl + Shift + I`).
3. Go to the "Application" tab.
4. Under "Storage" > "Cookies", click on "https://gemini.google.com".
5. Find the cookies with the name `__Secure-1PSID` (Must) and `__Secure-1PSIDTS` (Optional), and copy the values.

Now you can use the Google Gemini CLI:

> Supported options: `-s` (__Secure-1PSID cookie), `-t` (__Secure-1PSIDTS cookie), `-m` (if present, it'll print other Gemini's responses for your prompt), `-p` (if present with path, it'll save your chat history as markdown.), `-e` (if present with .env file location, it'll use that session cookie)

It'll save as your first prompt message. (eg: "Hey yo" -> gemini_hey_yo.md)

```
bard-rs --psid <your_psid> --psidts <your_psidts> --path ./

bard-rs --psid <your_psid> --path ./
```

Replace `<your_psid>` and `<your_psidts>` with the value you copied from the Developer Tools.

If you don't want to save the chat history as a Markdown file, skip `--path`:

```
bard-rs -p <your_psid>
```

If you don't want to pass that long session in terminal, use `.env` file (refer to `.env_sample`)

```
bard-rs -e .env -p ./
```

If you prefer not to specify a path, `bard-rs` will automatically search for the .env file in the following locations: the argument-provided path, the current working directory, and the directory of the bard-rs binary.

(`-p` is still required if you want to save the chat history as markdown file.)

```
bard-rs
```

above command is same as `bard-rs -e .env`

`.env` file must contain `PSID` key. (it is derived from `__Secure-1PSID`)

> ![IMPORTANT]
> Need `__Secure-1PSID` and `__Secure-1PSIDTS`

> ! using `echo PSID=... > .env` might cause encoding problem that `dotenv` cannot read and end up causing no session key error.

```
PSID=~.
```

## Resolving "SNlM0e not found" Error
> [!IMPORTANT]
> if you are getting "SNlM0e not found. Check your cookies." even with `__Secure-1PSIDTS`

1. Clear your cookies. (In Chrome, DevTools - Application - Cookies)
2. Login again.
3. Obtain new `__Secure-1PSID` (Seems like typically contains a numerical identifier representing the logged-in user.)

## Commands

- Type your message and press Enter to send it to Google Gemini.
- Type `!reset` to reset the conversation.
- Type `!exit` to exit the CLI.
- Type `!show` to see other Gemini's answers for your last message.

## License

This project is licensed under the [MIT License](LICENSE).


Credits:
- [acheong08](https://github.com/acheong08) - Inspired by this Python version.
