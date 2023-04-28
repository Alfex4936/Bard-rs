# <img src="https://user-images.githubusercontent.com/2356749/235068474-5daddf05-54d6-4391-ae97-1a944aebdec6.png" style="height: 32px"> Google Bard CLI

A simple command line interface for interacting with Google Bard, written in Rust.

![image](https://user-images.githubusercontent.com/2356749/235067629-4cf86781-72bb-468f-9bd3-c495a31818f1.png)


## Prerequisites

You need to have Rust and Cargo installed on your system. If you don't have them, you can install them from the [official Rust website](https://www.rust-lang.org/tools/install).

## Installation

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


or install from cargo.

```bash
cargo install bard-rs
```

## Usage

Before using the Google Bard CLI, you need to obtain your session cookie. To get the session cookie, follow these steps:

1. Go to [Google Bard](https://bard.google.com/) in Chrome.
2. Open Chrome Developer Tools (F12 or `Ctrl + Shift + I`).
3. Go to the "Application" tab.
4. Under "Storage" > "Cookies", click on "https://bard.google.com".
5. Find the cookie with the name `__Secure-1PSID`, and copy its value. (it includes "." usually)

Now you can use the Google Bard CLI:

```
bard-rs --session <your_session_cookie>
```

Replace `<your_session_cookie>` with the value you copied from the Developer Tools.

## Commands

- Type your message and press Enter to send it to Google Bard.
- Type `!reset` to reset the conversation.
- Type `!exit` to exit the CLI.

## License

This project is licensed under the [MIT License](LICENSE).


Credits:
- [acheong08](https://github.com/acheong08) - Inspired by this Python version.