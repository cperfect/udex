#!/usr/bin/env bash
#
# statusline-command.sh -- Claude Code statusLine command
#
# Mirrors the shell PS1: user@host, cwd, git branch (+dirty marker).
# Receives the Claude Code status JSON on stdin.
# Outputs a single line with ANSI colour codes via printf.

set -u

input="$(cat)"
# --- Working directory from Claude context ---
cwd="$(printf '%s' "$input" | jq -r '.workspace.current_dir // .cwd // empty' 2>/dev/null)"
[ -z "$cwd" ] && cwd="$(pwd)"

# --- User: prefer GITHUB_USER env, fall back to whoami ---
if [ -n "${GITHUB_USER:-}" ]; then
  user_str="@${GITHUB_USER}"
else
  user_str="$(whoami 2>/dev/null || echo user)"
fi

# --- Git branch + dirty check (skip optional locks) ---
git_part=""
hide1="$(git -C "$cwd" config --get devcontainers-theme.hide-status 2>/dev/null || true)"
hide2="$(git -C "$cwd" config --get codespaces-theme.hide-status 2>/dev/null || true)"
if [ "${hide1}" != "1" ] && [ "${hide2}" != "1" ]; then
  BRANCH="$(git --no-optional-locks -C "$cwd" symbolic-ref --short HEAD 2>/dev/null \
    || git --no-optional-locks -C "$cwd" rev-parse --short HEAD 2>/dev/null \
    || true)"
  if [ -n "${BRANCH:-}" ]; then
    dirty=""
    show_dirty="$(git -C "$cwd" config --get devcontainers-theme.show-dirty 2>/dev/null || true)"
    if [ "${show_dirty}" = "1" ] && \
       git --no-optional-locks -C "$cwd" ls-files --error-unmatch -m --directory \
           --no-empty-directory -o --exclude-standard ":/*" >/dev/null 2>&1; then
      dirty=" \033[1;33m✗"
    fi
    git_part="$(printf '\033[0;36m(\033[1;31m%s%b\033[0;36m) ' "${BRANCH}" "${dirty}")"
  fi
fi

# --- Model and context window ---
model_name="$(printf '%s' "$input" | jq -r '.model.display_name // empty' 2>/dev/null)"
ctx_pct="$(printf '%s' "$input" | jq -r '.context_window.used_percentage // empty' 2>/dev/null)"
model_part=""
if [ -n "${model_name}" ]; then
  ctx_str=""
  [ -n "${ctx_pct}" ] && ctx_str=" ${ctx_pct}%"
  model_part="$(printf '\033[0;90m[%s%s] ' "${model_name}" "${ctx_str}")"
fi

# --- Assemble output ---
printf '\033[0;32m%s \033[0m\033[1;34m%s \033[0m%s%s\033[0m' \
  "${user_str}" "${cwd}" "${git_part}" "${model_part}"
