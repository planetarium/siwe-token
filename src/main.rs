use std::io::{self, Read};
use std::process::Command as ShellCommand;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{SecondsFormat, Utc};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Token payload ────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct TokenPayload {
    message: String,
    signature: String,
}

// ── CLI ──────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "siwe-token",
    version,
    about = "SIWE token CLI for agent authentication"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate a SIWE message (prints to stdout)
    Prepare {
        /// Ethereum address (0x...)
        #[arg(long)]
        address: String,
        /// Domain (e.g. app.example.com)
        #[arg(long)]
        domain: String,
        /// URI (e.g. https://app.example.com)
        #[arg(long)]
        uri: String,
        /// Time-to-live: 30m, 1h, 7d, etc.
        #[arg(long, default_value = "7d")]
        ttl: String,
        /// SIWE statement
        #[arg(long, default_value = "I accept the Terms of Service")]
        statement: String,
        /// EIP-155 chain ID
        #[arg(long, default_value_t = 1)]
        chain_id: u64,
    },

    /// Encode message + signature into a base64url token
    Encode {
        /// Message file path (reads stdin if omitted)
        #[arg(long)]
        message_file: Option<String>,
        /// Hex-encoded signature (0x...)
        #[arg(long)]
        signature: String,
    },

    /// Decode a token and print its fields
    Decode {
        /// base64url-encoded SIWE token
        token: String,
    },

    /// Verify token signature, print address (exit 1 on failure)
    Verify {
        /// base64url-encoded SIWE token
        token: String,
    },

    /// All-in-one: prepare + external sign + encode
    Auth {
        /// Ethereum address (0x...)
        #[arg(long)]
        address: String,
        /// Domain
        #[arg(long)]
        domain: String,
        /// URI
        #[arg(long)]
        uri: String,
        /// Shell command for signing.
        /// The SIWE message is passed via $SIWE_MESSAGE env var.
        /// Example: cast wallet sign --keystore k.json "$SIWE_MESSAGE"
        #[arg(long)]
        sign_command: String,
        /// Time-to-live
        #[arg(long, default_value = "7d")]
        ttl: String,
        /// SIWE statement
        #[arg(long, default_value = "I accept the Terms of Service")]
        statement: String,
        /// EIP-155 chain ID
        #[arg(long, default_value_t = 1)]
        chain_id: u64,
    },
}

// ── Helpers ──────────────────────────────────────────────────────

fn parse_ttl(s: &str) -> Result<chrono::Duration, String> {
    let s = s.trim();
    if let Some(v) = s.strip_suffix('d') {
        Ok(chrono::Duration::days(
            v.parse::<i64>().map_err(|_| format!("bad ttl: {s}"))?,
        ))
    } else if let Some(v) = s.strip_suffix('h') {
        Ok(chrono::Duration::hours(
            v.parse::<i64>().map_err(|_| format!("bad ttl: {s}"))?,
        ))
    } else if let Some(v) = s.strip_suffix('m') {
        Ok(chrono::Duration::minutes(
            v.parse::<i64>().map_err(|_| format!("bad ttl: {s}"))?,
        ))
    } else {
        Err(format!("bad ttl format: {s} (use 30m, 1h, 7d)"))
    }
}

fn make_message(
    address: &str,
    domain: &str,
    uri: &str,
    ttl: &str,
    statement: &str,
    chain_id: u64,
) -> Result<String, String> {
    let dur = parse_ttl(ttl)?;
    let now = Utc::now();
    let exp = now + dur;
    let nonce = Uuid::new_v4().simple().to_string();

    Ok(format!(
        "{domain} wants you to sign in with your Ethereum account:\n\
         {address}\n\
         \n\
         {statement}\n\
         \n\
         URI: {uri}\n\
         Version: 1\n\
         Chain ID: {chain_id}\n\
         Nonce: {nonce}\n\
         Issued At: {issued}\n\
         Expiration Time: {expires}",
        issued = now.to_rfc3339_opts(SecondsFormat::Millis, true),
        expires = exp.to_rfc3339_opts(SecondsFormat::Millis, true),
    ))
}

fn encode_token(message: &str, signature: &str) -> String {
    let json = serde_json::to_string(&TokenPayload {
        message: message.into(),
        signature: signature.into(),
    })
    .expect("JSON serialization cannot fail");
    URL_SAFE_NO_PAD.encode(json.as_bytes())
}

fn decode_token(token: &str) -> Result<TokenPayload, String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(token.trim())
        .map_err(|e| format!("bad base64url: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("bad json: {e}"))
}

fn read_message_input(file: Option<&str>) -> Result<String, String> {
    match file {
        Some(path) => std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}")),
        None => {
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| format!("stdin: {e}"))?;
            Ok(buf)
        }
    }
}

fn parse_sig_bytes(hex_sig: &str) -> Result<[u8; 65], String> {
    let hex_str = hex_sig.strip_prefix("0x").unwrap_or(hex_sig);
    let bytes = hex::decode(hex_str).map_err(|e| format!("bad signature hex: {e}"))?;
    if bytes.len() != 65 {
        return Err(format!(
            "signature must be 65 bytes, got {}",
            bytes.len()
        ));
    }
    let mut arr = [0u8; 65];
    arr.copy_from_slice(&bytes);
    // Normalize v value: EIP-155 uses 27/28, secp256k1 recovery uses 0/1
    if arr[64] >= 27 {
        arr[64] -= 27;
    }
    Ok(arr)
}

// ── Commands ─────────────────────────────────────────────────────

fn cmd_prepare(
    address: &str,
    domain: &str,
    uri: &str,
    ttl: &str,
    statement: &str,
    chain_id: u64,
) -> Result<(), String> {
    let msg = make_message(address, domain, uri, ttl, statement, chain_id)?;
    print!("{msg}");
    Ok(())
}

fn cmd_encode(message_file: Option<&str>, signature: &str) -> Result<(), String> {
    let msg = read_message_input(message_file)?;
    println!("{}", encode_token(&msg, signature));
    Ok(())
}

fn cmd_decode(token: &str) -> Result<(), String> {
    let p = decode_token(token)?;
    match p.message.parse::<siwe::Message>() {
        Ok(m) => {
            println!("Address:    0x{}", hex::encode(m.address));
            println!("Domain:     {}", m.domain);
            if let Some(s) = &m.statement {
                println!("Statement:  {s}");
            }
            println!("URI:        {}", m.uri);
            println!("Chain ID:   {}", m.chain_id);
            println!("Nonce:      {}", m.nonce);
            println!("Issued At:  {}", m.issued_at);
            if let Some(e) = &m.expiration_time {
                println!("Expires:    {e}");
            }
        }
        Err(e) => {
            eprintln!("warning: SIWE parse error: {e:?}");
            println!("Message:\n{}", p.message);
        }
    }
    println!("Signature:  {}", p.signature);
    Ok(())
}

fn cmd_verify(token: &str) -> Result<(), String> {
    let p = decode_token(token)?;
    let msg: siwe::Message = p
        .message
        .parse()
        .map_err(|e| format!("bad SIWE message: {e:?}"))?;

    let sig = parse_sig_bytes(&p.signature)?;
    msg.verify_eip191(&sig)
        .map_err(|e| format!("verification failed: {e:?}"))?;

    // Check expiration
    if let Some(exp) = &msg.expiration_time {
        if let Ok(t) = chrono::DateTime::parse_from_rfc3339(&exp.to_string()) {
            if Utc::now() > t {
                return Err("token expired".into());
            }
        }
    }

    println!("0x{}", hex::encode(msg.address));
    Ok(())
}

fn cmd_auth(
    address: &str,
    domain: &str,
    uri: &str,
    sign_command: &str,
    ttl: &str,
    statement: &str,
    chain_id: u64,
) -> Result<(), String> {
    let msg = make_message(address, domain, uri, ttl, statement, chain_id)?;

    let output = ShellCommand::new("sh")
        .arg("-c")
        .arg(sign_command)
        .env("SIWE_MESSAGE", &msg)
        .output()
        .map_err(|e| format!("sign command failed to start: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("sign command failed: {}", stderr.trim()));
    }

    let sig = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();
    if sig.is_empty() {
        return Err("sign command returned empty output".into());
    }

    println!("{}", encode_token(&msg, &sig));
    Ok(())
}

// ── Main ─────────────────────────────────────────────────────────

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Prepare {
            address,
            domain,
            uri,
            ttl,
            statement,
            chain_id,
        } => cmd_prepare(&address, &domain, &uri, &ttl, &statement, chain_id),

        Cmd::Encode {
            message_file,
            signature,
        } => cmd_encode(message_file.as_deref(), &signature),

        Cmd::Decode { token } => cmd_decode(&token),
        Cmd::Verify { token } => cmd_verify(&token),

        Cmd::Auth {
            address,
            domain,
            uri,
            sign_command,
            ttl,
            statement,
            chain_id,
        } => cmd_auth(&address, &domain, &uri, &sign_command, &ttl, &statement, chain_id),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
