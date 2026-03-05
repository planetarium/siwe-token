# siwe-token

A single-binary CLI tool for creating, encoding, decoding, and verifying [SIWE (EIP-4361)](https://eips.ethereum.org/EIPS/eip-4361) tokens — designed for LLM agents and headless environments that authenticate via external wallet CLIs.

SIWE tokens are base64url-encoded `{ message, signature }` pairs. Any service can independently verify the token by recovering the signer's address from the signature — no shared secret required.

## Install

One-line install (macOS / Linux):

```bash
curl -fsSL https://raw.githubusercontent.com/planetarium/siwe-token/main/install.sh | sh
```

Custom install directory:

```bash
curl -fsSL https://raw.githubusercontent.com/planetarium/siwe-token/main/install.sh | INSTALL_DIR=~/.local/bin sh
```

Via cargo:

```bash
cargo install --git https://github.com/planetarium/siwe-token
```

Or build from source:

```bash
git clone https://github.com/planetarium/siwe-token
cd siwe-token
cargo build --release
# Binary at target/release/siwe-token (1.3MB)
```

## Commands

### `prepare` — Generate a SIWE message

```bash
siwe-token prepare \
  --address 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 \
  --domain app.example.com \
  --uri https://app.example.com \
  --ttl 7d
```

Outputs the SIWE message text to stdout.

### `encode` — Assemble a token from message + signature

```bash
siwe-token encode \
  --message-file /tmp/siwe-msg.txt \
  --signature 0xda0e85...
```

Outputs the base64url token to stdout. Reads from stdin if `--message-file` is omitted.

### `decode` — Inspect a token

```bash
siwe-token decode eyJtZXNzYWdl...
```

```
Address:    0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266
Domain:     app.example.com
Statement:  I accept the Terms of Service
URI:        https://app.example.com
Chain ID:   1
Nonce:      e749d1c140844c86a279f3b5780e2bc4
Issued At:  2026-03-05T09:39:13.849Z
Expires:    2026-03-05T10:39:13.849Z
Signature:  0xda0e85...
```

### `verify` — Verify signature and check expiration

```bash
siwe-token verify eyJtZXNzYWdl...
# stdout: 0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266
# exit 0 on success, exit 1 on failure
```

### `auth` — All-in-one with external signer

```bash
siwe-token auth \
  --address 0xf39F... \
  --domain app.example.com \
  --uri https://app.example.com \
  --sign-command 'cast wallet sign --keystore key.json --password-file pass "$SIWE_MESSAGE"'
```

Generates the message, passes it to the sign command via `$SIWE_MESSAGE` env var, and outputs the final token. One command, done.

If the sign command outputs JSON (e.g. `{"signature":"0x..."}`), use `--sign-format json` to extract the signature automatically:

```bash
siwe-token auth \
  --address 0xD0e3... \
  --domain app.example.com \
  --uri https://app.example.com \
  --sign-command 'a2a-wallet sign --message "$SIWE_MESSAGE" --json' \
  --sign-format json
```

| `--sign-format` | Expected output | Default |
|-----------------|----------------|---------|
| `raw` | Hex signature string (`0x...`) | Yes |
| `json` | JSON with `"signature"` field (`{"signature":"0x..."}`) | |

## Usage with a2a-wallet

```bash
ADDRESS=$(a2a-wallet whoami | grep Wallet | awk '{print $2}')

siwe-token auth \
  --address "$ADDRESS" \
  --domain app.example.com \
  --uri https://app.example.com \
  --sign-command 'a2a-wallet sign --message "$SIWE_MESSAGE" --json' \
  --sign-format json
```

## Usage with Foundry (`cast`)

```bash
# Step by step
siwe-token prepare \
  --address $(cast wallet address --keystore key.json) \
  --domain app.example.com \
  --uri https://app.example.com > /tmp/msg.txt

cast wallet sign --keystore key.json --password-file pass \
  "$(cat /tmp/msg.txt)" > /tmp/sig.txt

siwe-token encode --message-file /tmp/msg.txt --signature "$(cat /tmp/sig.txt)"

# Or all-in-one
siwe-token auth \
  --address $(cast wallet address --keystore key.json) \
  --domain app.example.com \
  --uri https://app.example.com \
  --sign-command 'cast wallet sign --keystore key.json --password-file pass "$SIWE_MESSAGE"'
```

## Token Format

```
base64url(JSON.stringify({ message: "<SIWE message text>", signature: "0x..." }))
```

Servers decode and verify by:

1. base64url decode → JSON parse
2. Parse SIWE message (EIP-4361)
3. Recover address from EIP-191 signature
4. Compare recovered address with the one in the message
5. Check expiration / notBefore constraints

## License

MIT
