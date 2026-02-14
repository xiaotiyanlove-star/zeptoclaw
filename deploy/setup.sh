#!/bin/sh
set -eu

# ZeptoClaw Universal VPS Setup Script
# Usage: curl -fsSL https://zeptoclaw.com/setup.sh | sh
#    or: curl -fsSL https://zeptoclaw.com/setup.sh | sh -s -- --docker
#    or: bash deploy/setup.sh --help

# ─── Constants ────────────────────────────────────────────────────────────────

REPO="qhkm/zeptoclaw"
BINARY="zeptoclaw"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="${HOME}/.zeptoclaw"
SERVICE_NAME="zeptoclaw"
SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"
DOCKER_IMAGE="ghcr.io/qhkm/zeptoclaw:latest"
CONTAINER_NAME="zeptoclaw"

# ─── Colors (only if terminal) ───────────────────────────────────────────────

if [ -t 1 ]; then
  RED='\033[0;31m'
  GREEN='\033[0;32m'
  YELLOW='\033[1;33m'
  BLUE='\033[0;34m'
  BOLD='\033[1m'
  RESET='\033[0m'
else
  RED=''
  GREEN=''
  YELLOW=''
  BLUE=''
  BOLD=''
  RESET=''
fi

# ─── Helpers ──────────────────────────────────────────────────────────────────

info()  { printf "${BLUE}[info]${RESET}  %s\n" "$1"; }
ok()    { printf "${GREEN}[ok]${RESET}    %s\n" "$1"; }
warn()  { printf "${YELLOW}[warn]${RESET}  %s\n" "$1"; }
err()   { printf "${RED}[error]${RESET} %s\n" "$1" >&2; }
die()   { err "$1"; exit 1; }

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    die "Required command not found: $1"
  fi
}

need_sudo() {
  if [ "$(id -u)" -ne 0 ]; then
    if ! command -v sudo >/dev/null 2>&1; then
      die "This operation requires root. Please run as root or install sudo."
    fi
    SUDO="sudo"
  else
    SUDO=""
  fi
}

prompt_value() {
  # prompt_value "Prompt text" "default_value"
  _prompt="$1"
  _default="${2:-}"
  if [ -n "$_default" ]; then
    printf "%s [%s]: " "$_prompt" "$_default"
  else
    printf "%s: " "$_prompt"
  fi
  read -r _answer </dev/tty || _answer=""
  if [ -z "$_answer" ]; then
    _answer="$_default"
  fi
  printf '%s' "$_answer"
}

prompt_secret() {
  # prompt_secret "Prompt text"
  printf "%s: " "$1"
  # Try to disable echo; fall back to normal read
  if [ -t 0 ]; then
    stty -echo 2>/dev/null || true
    read -r _secret </dev/tty || _secret=""
    stty echo 2>/dev/null || true
    printf '\n'
  else
    read -r _secret || _secret=""
  fi
  printf '%s' "$_secret"
}

# ─── Usage ────────────────────────────────────────────────────────────────────

usage() {
  cat <<EOF
${BOLD}ZeptoClaw VPS Setup${RESET}

Usage:
  curl -fsSL https://zeptoclaw.com/setup.sh | sh
  curl -fsSL https://zeptoclaw.com/setup.sh | sh -s -- --docker
  bash deploy/setup.sh [OPTIONS]

Options:
  --docker      Use Docker instead of native binary
  --uninstall   Remove ZeptoClaw from this system
  --help        Show this help message

Environment variables (skip interactive prompts):
  ANTHROPIC_KEY     Anthropic API key
  OPENAI_KEY        OpenAI API key
  TELEGRAM_TOKEN    Telegram bot token
  DISCORD_TOKEN     Discord bot token
  SLACK_BOT_TOKEN   Slack bot token
  SLACK_APP_TOKEN   Slack app token

Examples:
  # Non-interactive binary install
  ANTHROPIC_KEY=sk-ant-... TELEGRAM_TOKEN=123:ABC... \\
    curl -fsSL https://zeptoclaw.com/setup.sh | sh

  # Docker mode
  curl -fsSL https://zeptoclaw.com/setup.sh | sh -s -- --docker

  # Uninstall
  bash deploy/setup.sh --uninstall
EOF
}

# ─── System checks ───────────────────────────────────────────────────────────

check_system() {
  OS="$(uname -s)"
  case "$OS" in
    Linux) ;;
    *) die "This setup script is for Linux only. Detected: $OS" ;;
  esac

  ARCH="$(uname -m)"
  case "$ARCH" in
    x86_64|amd64)   ARCH_LABEL="x86_64" ;;
    aarch64|arm64)   ARCH_LABEL="aarch64" ;;
    *) die "Unsupported architecture: $ARCH" ;;
  esac

  # Detect distro family
  DISTRO="unknown"
  if [ -f /etc/os-release ]; then
    . /etc/os-release
    case "${ID:-}" in
      ubuntu|debian|pop|linuxmint|elementary|zorin)
        DISTRO="debian"
        ;;
      rhel|centos|fedora|rocky|alma|amzn|ol)
        DISTRO="rhel"
        ;;
      *)
        # Check ID_LIKE as fallback
        case "${ID_LIKE:-}" in
          *debian*|*ubuntu*) DISTRO="debian" ;;
          *rhel*|*fedora*|*centos*) DISTRO="rhel" ;;
        esac
        ;;
    esac
  fi

  if [ "$DISTRO" = "unknown" ]; then
    warn "Could not detect Linux distribution. Proceeding anyway."
  fi

  info "System: Linux/${ARCH_LABEL} (${DISTRO})"

  # Check for required commands
  need_cmd curl
}

# ─── Uninstall ───────────────────────────────────────────────────────────────

do_uninstall() {
  info "Uninstalling ZeptoClaw..."
  need_sudo

  # Stop and disable systemd service
  if [ -f "$SERVICE_FILE" ]; then
    info "Stopping systemd service..."
    $SUDO systemctl stop "$SERVICE_NAME" 2>/dev/null || true
    $SUDO systemctl disable "$SERVICE_NAME" 2>/dev/null || true
    $SUDO rm -f "$SERVICE_FILE"
    $SUDO systemctl daemon-reload 2>/dev/null || true
    ok "Systemd service removed"
  fi

  # Remove binary
  if [ -f "${INSTALL_DIR}/${BINARY}" ]; then
    $SUDO rm -f "${INSTALL_DIR}/${BINARY}"
    ok "Binary removed from ${INSTALL_DIR}"
  fi

  # Stop and remove Docker container
  if command -v docker >/dev/null 2>&1; then
    if docker ps -a --format '{{.Names}}' 2>/dev/null | grep -q "^${CONTAINER_NAME}$"; then
      info "Stopping Docker container..."
      docker stop "$CONTAINER_NAME" 2>/dev/null || true
      docker rm "$CONTAINER_NAME" 2>/dev/null || true
      ok "Docker container removed"
    fi
    # Remove image
    if docker images --format '{{.Repository}}:{{.Tag}}' 2>/dev/null | grep -q "^${DOCKER_IMAGE}$"; then
      docker rmi "$DOCKER_IMAGE" 2>/dev/null || true
      ok "Docker image removed"
    fi
  fi

  # Ask about config directory
  if [ -d "$CONFIG_DIR" ]; then
    printf "Remove config directory %s? [y/N]: " "$CONFIG_DIR"
    read -r _remove </dev/tty 2>/dev/null || _remove="n"
    case "$_remove" in
      [yY]|[yY][eE][sS])
        rm -rf "$CONFIG_DIR"
        ok "Config directory removed"
        ;;
      *)
        info "Config directory preserved at ${CONFIG_DIR}"
        ;;
    esac
  fi

  ok "ZeptoClaw has been uninstalled"
  exit 0
}

# ─── Binary install ──────────────────────────────────────────────────────────

install_binary() {
  ARTIFACT="${BINARY}-linux-${ARCH_LABEL}"
  BASE_URL="https://github.com/${REPO}/releases/latest/download"

  need_sudo

  # Check for pre-existing install
  if [ -f "${INSTALL_DIR}/${BINARY}" ]; then
    warn "Existing installation found at ${INSTALL_DIR}/${BINARY}"
    EXISTING_VERSION="$(${INSTALL_DIR}/${BINARY} --version 2>/dev/null || echo 'unknown')"
    info "Existing version: ${EXISTING_VERSION}"
    info "Upgrading to latest..."
  fi

  # Create temp directory
  TMP_DIR="$(mktemp -d)"
  trap 'rm -rf "$TMP_DIR"' EXIT

  info "Downloading ${ARTIFACT}..."
  curl -fsSL "${BASE_URL}/${ARTIFACT}" -o "${TMP_DIR}/${BINARY}" || \
    die "Failed to download binary. Check your internet connection and that releases exist at ${BASE_URL}/${ARTIFACT}"
  curl -fsSL "${BASE_URL}/${ARTIFACT}.sha256" -o "${TMP_DIR}/${BINARY}.sha256" || \
    die "Failed to download checksum file"

  # Verify checksum
  info "Verifying SHA256 checksum..."
  cd "$TMP_DIR"
  EXPECTED="$(awk '{print $1}' "${BINARY}.sha256")"
  if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL="$(sha256sum "${BINARY}" | awk '{print $1}')"
  elif command -v shasum >/dev/null 2>&1; then
    ACTUAL="$(shasum -a 256 "${BINARY}" | awk '{print $1}')"
  else
    warn "No checksum tool found (sha256sum or shasum). Skipping verification."
    ACTUAL="$EXPECTED"
  fi

  if [ "$EXPECTED" != "$ACTUAL" ]; then
    die "Checksum verification failed!\n  Expected: ${EXPECTED}\n  Actual:   ${ACTUAL}"
  fi
  ok "Checksum verified"

  # Install binary
  chmod +x "${TMP_DIR}/${BINARY}"
  if [ -w "$INSTALL_DIR" ]; then
    mv "${TMP_DIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
  else
    $SUDO mv "${TMP_DIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
  fi
  ok "Binary installed to ${INSTALL_DIR}/${BINARY}"
}

# ─── Docker install ──────────────────────────────────────────────────────────

install_docker() {
  # Install Docker if not present
  if ! command -v docker >/dev/null 2>&1; then
    info "Docker not found. Installing Docker CE..."
    need_sudo
    curl -fsSL https://get.docker.com | $SUDO sh || \
      die "Failed to install Docker. Please install it manually: https://docs.docker.com/engine/install/"
    # Add current user to docker group if not root
    if [ "$(id -u)" -ne 0 ]; then
      $SUDO usermod -aG docker "$(whoami)" 2>/dev/null || true
      warn "You may need to log out and back in for Docker group membership to take effect."
    fi
    ok "Docker installed"
  else
    ok "Docker already installed"
  fi

  # Verify Docker is running
  if ! docker info >/dev/null 2>&1; then
    need_sudo
    $SUDO systemctl start docker 2>/dev/null || \
      $SUDO service docker start 2>/dev/null || \
      die "Docker is installed but not running. Please start Docker and re-run this script."
  fi

  # Pull image
  info "Pulling ${DOCKER_IMAGE}..."
  docker pull "$DOCKER_IMAGE" || \
    die "Failed to pull Docker image. Check your internet connection."
  ok "Docker image pulled"
}

# ─── Interactive config wizard ───────────────────────────────────────────────

config_wizard() {
  # Results stored in these variables
  PROVIDER_ANTHROPIC_KEY="${ANTHROPIC_KEY:-}"
  PROVIDER_OPENAI_KEY="${OPENAI_KEY:-}"
  CHANNEL_TELEGRAM_TOKEN="${TELEGRAM_TOKEN:-}"
  CHANNEL_DISCORD_TOKEN="${DISCORD_TOKEN:-}"
  CHANNEL_SLACK_BOT="${SLACK_BOT_TOKEN:-}"
  CHANNEL_SLACK_APP="${SLACK_APP_TOKEN:-}"
  CHOSEN_CHANNEL=""

  # Skip wizard if all essential env vars are set
  if [ -n "$PROVIDER_ANTHROPIC_KEY" ] || [ -n "$PROVIDER_OPENAI_KEY" ]; then
    if [ -n "$CHANNEL_TELEGRAM_TOKEN" ] || [ -n "$CHANNEL_DISCORD_TOKEN" ] || \
       [ -n "$CHANNEL_SLACK_BOT" ]; then
      info "Configuration detected from environment variables. Skipping wizard."
      # Determine channel from env
      if [ -n "$CHANNEL_TELEGRAM_TOKEN" ]; then
        CHOSEN_CHANNEL="telegram"
      elif [ -n "$CHANNEL_DISCORD_TOKEN" ]; then
        CHOSEN_CHANNEL="discord"
      elif [ -n "$CHANNEL_SLACK_BOT" ]; then
        CHOSEN_CHANNEL="slack"
      fi
      return 0
    fi
  fi

  printf "\n${BOLD}=== ZeptoClaw Configuration Wizard ===${RESET}\n\n"

  # ── LLM Provider ──
  if [ -z "$PROVIDER_ANTHROPIC_KEY" ] && [ -z "$PROVIDER_OPENAI_KEY" ]; then
    printf "Choose LLM provider:\n"
    printf "  ${BOLD}1${RESET}) Anthropic (Claude) — ${GREEN}default${RESET}\n"
    printf "  ${BOLD}2${RESET}) OpenAI (GPT)\n"
    printf "  ${BOLD}3${RESET}) Both\n"
    PROVIDER_CHOICE="$(prompt_value "Selection" "1")"

    case "$PROVIDER_CHOICE" in
      1|"")
        PROVIDER_ANTHROPIC_KEY="$(prompt_secret "Enter Anthropic API key (sk-ant-...)")"
        [ -z "$PROVIDER_ANTHROPIC_KEY" ] && die "Anthropic API key is required"
        ;;
      2)
        PROVIDER_OPENAI_KEY="$(prompt_secret "Enter OpenAI API key (sk-...)")"
        [ -z "$PROVIDER_OPENAI_KEY" ] && die "OpenAI API key is required"
        ;;
      3)
        PROVIDER_ANTHROPIC_KEY="$(prompt_secret "Enter Anthropic API key (sk-ant-...)")"
        [ -z "$PROVIDER_ANTHROPIC_KEY" ] && die "Anthropic API key is required"
        PROVIDER_OPENAI_KEY="$(prompt_secret "Enter OpenAI API key (sk-...)")"
        [ -z "$PROVIDER_OPENAI_KEY" ] && die "OpenAI API key is required"
        ;;
      *) die "Invalid selection: $PROVIDER_CHOICE" ;;
    esac
  else
    info "LLM provider key(s) detected from environment"
  fi
  printf "\n"

  # ── Channel ──
  if [ -z "$CHANNEL_TELEGRAM_TOKEN" ] && [ -z "$CHANNEL_DISCORD_TOKEN" ] && \
     [ -z "$CHANNEL_SLACK_BOT" ]; then
    printf "Choose messaging channel:\n"
    printf "  ${BOLD}1${RESET}) Telegram — ${GREEN}default${RESET}\n"
    printf "  ${BOLD}2${RESET}) Discord\n"
    printf "  ${BOLD}3${RESET}) Slack\n"
    printf "  ${BOLD}4${RESET}) Webhook only (HTTP POST inbound)\n"
    printf "  ${BOLD}5${RESET}) Skip (configure later)\n"
    CHANNEL_CHOICE="$(prompt_value "Selection" "1")"

    case "$CHANNEL_CHOICE" in
      1|"")
        CHOSEN_CHANNEL="telegram"
        CHANNEL_TELEGRAM_TOKEN="$(prompt_secret "Enter Telegram bot token (from @BotFather)")"
        [ -z "$CHANNEL_TELEGRAM_TOKEN" ] && die "Telegram bot token is required"
        ;;
      2)
        CHOSEN_CHANNEL="discord"
        CHANNEL_DISCORD_TOKEN="$(prompt_secret "Enter Discord bot token")"
        [ -z "$CHANNEL_DISCORD_TOKEN" ] && die "Discord bot token is required"
        ;;
      3)
        CHOSEN_CHANNEL="slack"
        CHANNEL_SLACK_BOT="$(prompt_secret "Enter Slack bot token (xoxb-...)")"
        [ -z "$CHANNEL_SLACK_BOT" ] && die "Slack bot token is required"
        CHANNEL_SLACK_APP="$(prompt_secret "Enter Slack app token (xapp-...)")"
        [ -z "$CHANNEL_SLACK_APP" ] && die "Slack app token is required"
        ;;
      4)
        CHOSEN_CHANNEL="webhook"
        ;;
      5)
        CHOSEN_CHANNEL=""
        info "No channel configured. You can set one up later in ${CONFIG_DIR}/config.json"
        ;;
      *) die "Invalid selection: $CHANNEL_CHOICE" ;;
    esac
  else
    info "Channel credentials detected from environment"
    if [ -n "$CHANNEL_TELEGRAM_TOKEN" ]; then
      CHOSEN_CHANNEL="telegram"
    elif [ -n "$CHANNEL_DISCORD_TOKEN" ]; then
      CHOSEN_CHANNEL="discord"
    elif [ -n "$CHANNEL_SLACK_BOT" ]; then
      CHOSEN_CHANNEL="slack"
    fi
  fi
}

# ─── Write config (binary mode) ─────────────────────────────────────────────

write_config_json() {
  mkdir -p "$CONFIG_DIR"

  # Build providers JSON
  PROVIDERS_JSON='{'
  FIRST_PROVIDER=true

  if [ -n "$PROVIDER_ANTHROPIC_KEY" ]; then
    PROVIDERS_JSON="${PROVIDERS_JSON}"'
    "anthropic": {
      "api_key": "'"$PROVIDER_ANTHROPIC_KEY"'"
    }'
    FIRST_PROVIDER=false
  fi

  if [ -n "$PROVIDER_OPENAI_KEY" ]; then
    if [ "$FIRST_PROVIDER" = false ]; then
      PROVIDERS_JSON="${PROVIDERS_JSON},"
    fi
    PROVIDERS_JSON="${PROVIDERS_JSON}"'
    "openai": {
      "api_key": "'"$PROVIDER_OPENAI_KEY"'"
    }'
  fi

  PROVIDERS_JSON="${PROVIDERS_JSON}"'
  }'

  # Build channels JSON
  CHANNELS_JSON='{'
  case "$CHOSEN_CHANNEL" in
    telegram)
      CHANNELS_JSON="${CHANNELS_JSON}"'
    "telegram": {
      "bot_token": "'"$CHANNEL_TELEGRAM_TOKEN"'"
    }'
      ;;
    discord)
      CHANNELS_JSON="${CHANNELS_JSON}"'
    "discord": {
      "bot_token": "'"$CHANNEL_DISCORD_TOKEN"'"
    }'
      ;;
    slack)
      CHANNELS_JSON="${CHANNELS_JSON}"'
    "slack": {
      "bot_token": "'"$CHANNEL_SLACK_BOT"'",
      "app_token": "'"$CHANNEL_SLACK_APP"'"
    }'
      ;;
    webhook)
      CHANNELS_JSON="${CHANNELS_JSON}"'
    "webhook": {
      "enabled": true,
      "port": 8080
    }'
      ;;
  esac
  CHANNELS_JSON="${CHANNELS_JSON}"'
  }'

  CONFIG_FILE="${CONFIG_DIR}/config.json"

  # Preserve existing config if present
  if [ -f "$CONFIG_FILE" ]; then
    cp "$CONFIG_FILE" "${CONFIG_FILE}.bak"
    warn "Existing config backed up to ${CONFIG_FILE}.bak"
  fi

  cat > "$CONFIG_FILE" <<CONFIGEOF
{
  "providers": ${PROVIDERS_JSON},
  "channels": ${CHANNELS_JSON}
}
CONFIGEOF

  # Secure permissions — config contains API keys
  chmod 600 "$CONFIG_FILE"
  ok "Config written to ${CONFIG_FILE} (mode 600)"
}

# ─── Write env file (Docker mode) ───────────────────────────────────────────

write_deploy_env() {
  mkdir -p "$CONFIG_DIR"
  ENV_FILE="${CONFIG_DIR}/deploy.env"

  # Preserve existing env file if present
  if [ -f "$ENV_FILE" ]; then
    cp "$ENV_FILE" "${ENV_FILE}.bak"
    warn "Existing deploy.env backed up to ${ENV_FILE}.bak"
  fi

  cat > "$ENV_FILE" <<ENVEOF
# ZeptoClaw Docker environment — generated by setup.sh
RUST_LOG=zeptoclaw=info
ENVEOF

  if [ -n "$PROVIDER_ANTHROPIC_KEY" ]; then
    printf 'ZEPTOCLAW_PROVIDERS_ANTHROPIC_API_KEY=%s\n' "$PROVIDER_ANTHROPIC_KEY" >> "$ENV_FILE"
  fi
  if [ -n "$PROVIDER_OPENAI_KEY" ]; then
    printf 'ZEPTOCLAW_PROVIDERS_OPENAI_API_KEY=%s\n' "$PROVIDER_OPENAI_KEY" >> "$ENV_FILE"
  fi
  if [ -n "$CHANNEL_TELEGRAM_TOKEN" ]; then
    printf 'ZEPTOCLAW_CHANNELS_TELEGRAM_BOT_TOKEN=%s\n' "$CHANNEL_TELEGRAM_TOKEN" >> "$ENV_FILE"
  fi
  if [ -n "$CHANNEL_DISCORD_TOKEN" ]; then
    printf 'ZEPTOCLAW_CHANNELS_DISCORD_BOT_TOKEN=%s\n' "$CHANNEL_DISCORD_TOKEN" >> "$ENV_FILE"
  fi
  if [ -n "$CHANNEL_SLACK_BOT" ]; then
    printf 'ZEPTOCLAW_CHANNELS_SLACK_BOT_TOKEN=%s\n' "$CHANNEL_SLACK_BOT" >> "$ENV_FILE"
  fi
  if [ -n "$CHANNEL_SLACK_APP" ]; then
    printf 'ZEPTOCLAW_CHANNELS_SLACK_APP_TOKEN=%s\n' "$CHANNEL_SLACK_APP" >> "$ENV_FILE"
  fi

  # Secure permissions — env file contains API keys
  chmod 600 "$ENV_FILE"
  ok "Environment written to ${ENV_FILE} (mode 600)"
}

# ─── Systemd service (binary mode) ──────────────────────────────────────────

create_systemd_service() {
  need_sudo

  info "Creating systemd service..."

  $SUDO tee "$SERVICE_FILE" >/dev/null <<SERVICEEOF
[Unit]
Description=ZeptoClaw AI Agent
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$(whoami)
ExecStart=${INSTALL_DIR}/${BINARY} gateway
WorkingDirectory=${HOME}
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
Environment=RUST_LOG=zeptoclaw=info

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=${CONFIG_DIR}

[Install]
WantedBy=multi-user.target
SERVICEEOF

  $SUDO systemctl daemon-reload
  $SUDO systemctl enable "$SERVICE_NAME" >/dev/null 2>&1
  $SUDO systemctl start "$SERVICE_NAME"
  ok "Systemd service created and started"
}

# ─── Start Docker container ─────────────────────────────────────────────────

start_docker_container() {
  # Stop existing container if running
  if docker ps -a --format '{{.Names}}' 2>/dev/null | grep -q "^${CONTAINER_NAME}$"; then
    info "Stopping existing container..."
    docker stop "$CONTAINER_NAME" 2>/dev/null || true
    docker rm "$CONTAINER_NAME" 2>/dev/null || true
  fi

  ENV_FILE="${CONFIG_DIR}/deploy.env"

  info "Starting Docker container..."
  docker run -d \
    --name "$CONTAINER_NAME" \
    --restart unless-stopped \
    --env-file "$ENV_FILE" \
    -v "${CONFIG_DIR}:/data" \
    -p 8080:8080 \
    -p 9090:9090 \
    --memory=128m \
    --cpus=0.5 \
    "$DOCKER_IMAGE" \
    zeptoclaw gateway || die "Failed to start Docker container"

  ok "Docker container started"
}

# ─── Verify ──────────────────────────────────────────────────────────────────

verify_install() {
  _mode="$1"  # "binary" or "docker"

  info "Waiting for service to start..."
  sleep 2

  printf "\n${BOLD}=== Installation Complete ===${RESET}\n\n"

  if [ "$_mode" = "binary" ]; then
    if $SUDO systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
      ok "Service is running"
    else
      warn "Service may not be running yet. Check with: sudo systemctl status ${SERVICE_NAME}"
    fi

    VERSION="$(${INSTALL_DIR}/${BINARY} --version 2>/dev/null || echo 'installed')"
    printf "\n"
    info "Version:  ${VERSION}"
    info "Binary:   ${INSTALL_DIR}/${BINARY}"
    info "Config:   ${CONFIG_DIR}/config.json"
    info "Service:  ${SERVICE_FILE}"
    printf "\n${BOLD}Useful commands:${RESET}\n"
    printf "  sudo systemctl status %s    # Check status\n" "$SERVICE_NAME"
    printf "  sudo journalctl -u %s -f    # Follow logs\n" "$SERVICE_NAME"
    printf "  sudo systemctl restart %s   # Restart\n" "$SERVICE_NAME"
    printf "  sudo systemctl stop %s      # Stop\n" "$SERVICE_NAME"
    printf "  %s agent -m 'Hello'         # Test agent\n" "$BINARY"
    printf "  nano %s/config.json         # Edit config\n" "$CONFIG_DIR"

  elif [ "$_mode" = "docker" ]; then
    if docker ps --format '{{.Names}}' 2>/dev/null | grep -q "^${CONTAINER_NAME}$"; then
      ok "Container is running"
    else
      warn "Container may not be running. Check with: docker logs ${CONTAINER_NAME}"
    fi

    printf "\n"
    info "Image:    ${DOCKER_IMAGE}"
    info "Config:   ${CONFIG_DIR}/deploy.env"
    info "Data:     ${CONFIG_DIR} (mounted at /data)"
    printf "\n${BOLD}Useful commands:${RESET}\n"
    printf "  docker logs -f %s             # Follow logs\n" "$CONTAINER_NAME"
    printf "  docker restart %s             # Restart\n" "$CONTAINER_NAME"
    printf "  docker stop %s                # Stop\n" "$CONTAINER_NAME"
    printf "  docker exec -it %s sh         # Shell into container\n" "$CONTAINER_NAME"
    printf "  nano %s/deploy.env            # Edit config\n" "$CONFIG_DIR"
    printf "  docker pull %s && \\\\\n" "$DOCKER_IMAGE"
    printf "    docker stop %s && docker rm %s && \\\\\n" "$CONTAINER_NAME" "$CONTAINER_NAME"
    printf "    bash deploy/setup.sh --docker  # Upgrade\n"
  fi

  printf "\n${BOLD}Docs:${RESET} https://github.com/${REPO}\n\n"
}

# ─── Main ────────────────────────────────────────────────────────────────────

main() {
  MODE="binary"

  # Parse arguments
  while [ $# -gt 0 ]; do
    case "$1" in
      --docker)
        MODE="docker"
        shift
        ;;
      --uninstall)
        check_system
        do_uninstall
        ;;
      --help|-h)
        usage
        exit 0
        ;;
      *)
        die "Unknown option: $1 (see --help)"
        ;;
    esac
  done

  printf "\n${BOLD}ZeptoClaw VPS Setup${RESET} (%s mode)\n\n" "$MODE"

  check_system

  # Install
  if [ "$MODE" = "docker" ]; then
    install_docker
  else
    install_binary
  fi

  printf "\n"

  # Configure
  config_wizard

  printf "\n"

  # Write config and start
  if [ "$MODE" = "docker" ]; then
    write_deploy_env
    start_docker_container
  else
    write_config_json
    create_systemd_service
  fi

  # Verify
  verify_install "$MODE"
}

main "$@"
