#!/usr/bin/env bash
# System-info probe for the Terminal Assistant (Bash).
#
# Emits a compact, stable, machine-readable report describing the host and the
# availability/flavour of common command-line utilities. The assistant reads
# this to tailor its command suggestions (e.g. GNU vs BSD `sed -i` syntax, which
# tools exist, which package manager to use). Output is plain `key: value` lines
# grouped under `# section` headers; missing tools are reported explicitly so the
# assistant never has to guess.
#
# Runs in a discarded subshell, so names need no guarding. Stays within
# POSIX-ish Bash + coreutils; every optional tool is probed defensively.

# Print "key: value".
kv() { printf '%s: %s\n' "$1" "$2"; }

# Echo the resolved path of a command, or empty if absent.
which_() { command -v "$1" 2>/dev/null; }

# Detect whether a tool is GNU by checking `--version` output. Prints
# "gnu", "bsd/other", or "absent".
flavour() {
  local bin
  bin=$(which_ "$1") || true
  if [ -z "$bin" ]; then
    printf 'absent'
    return
  fi
  if "$bin" --version >/dev/null 2>&1 && "$bin" --version 2>&1 | grep -qi 'GNU'; then
    printf 'gnu'
  else
    printf 'bsd/other'
  fi
}

# List which of the given commands are present. Prints two lines:
#   <label>.present: a b c
#   <label>.missing: x y z
probe_group() {
  local label=$1
  shift
  local present="" missing=""
  local cmd
  for cmd in "$@"; do
    if command -v "$cmd" >/dev/null 2>&1; then
      present="$present $cmd"
    else
      missing="$missing $cmd"
    fi
  done
  kv "${label}.present" "${present# }"
  kv "${label}.missing" "${missing# }"
}

echo "# host"
kv "os"            "$(uname -s 2>/dev/null || echo unknown)"
kv "kernel"        "$(uname -r 2>/dev/null || echo unknown)"
kv "arch"          "$(uname -m 2>/dev/null || echo unknown)"
kv "hostname"      "$(uname -n 2>/dev/null || echo unknown)"

# Distro / product details where available.
if [ -r /etc/os-release ]; then
  # shellcheck disable=SC1091
  . /etc/os-release
  kv "distro"      "${PRETTY_NAME:-${NAME:-unknown}}"
  kv "distro.id"   "${ID:-unknown}"
elif [ "$(uname -s 2>/dev/null)" = "Darwin" ]; then
  kv "distro"      "macOS $(sw_vers -productVersion 2>/dev/null || echo '?')"
  kv "distro.id"   "macos"
else
  kv "distro"      "unknown"
  kv "distro.id"   "unknown"
fi

echo
echo "# session"
kv "shell"         "bash"
kv "shell.version" "${BASH_VERSION:-unknown}"
kv "user"          "$(id -un 2>/dev/null || echo "${USER:-unknown}")"
kv "uid"           "$(id -u 2>/dev/null || echo unknown)"
kv "home"          "${HOME:-unknown}"

echo
echo "# coreutil flavour (affects flag syntax)"
kv "sed"           "$(flavour sed)"
kv "grep"          "$(flavour grep)"
kv "awk"           "$(flavour awk)"
kv "date"          "$(flavour date)"
kv "readlink"      "$(flavour readlink)"
kv "getopt"        "$(flavour getopt)"

echo
echo "# utilities"
probe_group "core"       ls cat cp mv rm mkdir grep sed awk find cut sort uniq head tail tr xargs tee wc
probe_group "modern"     rg fd bat eza exa fzf jq yq delta zoxide tree
probe_group "net"        curl wget ssh scp rsync nc dig host ping
probe_group "vcs"        git gh hg svn
probe_group "editors"    vim nvim nano emacs code
probe_group "archive"    tar gzip bzip2 xz zstd zip unzip 7z
probe_group "build"      make cmake gcc clang pkg-config
probe_group "langs"      python3 python node deno bun ruby go rustc cargo java php
probe_group "jspkg"      npm pnpm yarn
probe_group "pkgmgr"     apt apt-get dnf yum pacman zypper apk brew port nix snap flatpak
probe_group "containers" docker podman kubectl helm docker-compose
probe_group "sysctl"     systemctl service launchctl journalctl
